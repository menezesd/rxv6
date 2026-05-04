//! Physical page allocator.
//!
//! Maintains two pools of physical pages (kernel and user), each tracked
//! by a bitmap. Ported from Pintos palloc.c.

use bitflags::bitflags;
use crate::sync::InterruptGuard;
use super::bitmap::Bitmap;
use super::vaddr::{PGSIZE, PHYS_BASE, pg_round_up, ptov};

bitflags! {
    /// Flags for page allocation requests.
    pub struct PallocFlags: u32 {
        /// Panic if allocation fails.
        const ASSERT = 0x01;
        /// Zero the allocated page(s).
        const ZERO   = 0x02;
        /// Allocate from the user pool (otherwise kernel pool).
        const USER   = 0x04;
    }
}

/// A pool of allocatable physical pages.
struct Pool {
    bitmap: Bitmap,
    /// Kernel virtual address of the first allocatable page in this pool.
    base: *mut u8,
}

// Safety: Pool contains a raw pointer (base) but is only accessed with interrupts
// disabled (InterruptGuard) in get_multiple/free_multiple, ensuring no concurrent access.
unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}

impl Pool {
    const fn empty() -> Self {
        Pool {
            bitmap: Bitmap::empty(),
            base: core::ptr::null_mut(),
        }
    }
}

static mut KERNEL_POOL: Pool = Pool::empty();
static mut USER_POOL: Pool = Pool::empty();

// External symbols defined in assembly / linker script.
extern "C" {
    static init_ram_pages: u32;
    static multiboot_info_phys: u32;
    static _end: u8;
}

/// Maximum RAM we can use. Boot page tables only map 64MB via PSE pages.
/// After paging_init sets up proper 4KB page tables for all RAM, we could
/// expand, but 64MB is plenty for an educational OS.
const MAX_RAM_PAGES: usize = 64 * 1024 * 1024 / PGSIZE; // 16384 pages = 64MB

/// Read the total number of RAM pages. First try `init_ram_pages` (set by
/// assembly); if still zero, parse multiboot `mem_upper` to compute it.
/// Capped at 64MB (the boot page table mapping limit).
pub fn detect_ram_pages() -> usize {
    let pages = unsafe {
        let pages = core::ptr::read_volatile(&raw const init_ram_pages) as usize;
        if pages != 0 {
            return pages.min(MAX_RAM_PAGES);
        }

        // Parse multiboot info structure.
        // multiboot_info_phys is a *physical* address; convert to virtual.
        let mb_phys = core::ptr::read_volatile(&raw const multiboot_info_phys) as usize;
        if mb_phys == 0 {
            return MAX_RAM_PAGES;
        }
        let mb_virt = mb_phys + PHYS_BASE;
        let flags = *(mb_virt as *const u32);
        if flags & 1 != 0 {
            // mem_lower at offset 4 (KB below 1 MB), mem_upper at offset 8.
            let mem_upper_kb = *((mb_virt + 8) as *const u32) as usize;
            // Total memory = lower 1 MB + mem_upper KB above 1 MB.
            let total_bytes = (1024 + mem_upper_kb) * 1024;
            total_bytes / PGSIZE
        } else {
            MAX_RAM_PAGES
        }
    };
    pages.min(MAX_RAM_PAGES)
}

/// Initialise the page allocator.
///
/// `user_page_limit`: maximum number of user pages (0 = no limit).
/// Must be called before any page allocations.
pub fn init(user_page_limit: usize) {
    unsafe {
        let ram_pages = detect_ram_pages();

        // Free memory starts right after the kernel image.
        let free_start = pg_round_up(&raw const _end as usize);
        let ram_end = ptov(ram_pages * PGSIZE);

        // Pages available for allocation.
        let free_pages = (ram_end - free_start) / PGSIZE;

        // Split roughly in half between kernel and user pools.
        let mut user_pages = free_pages / 2;
        if user_page_limit > 0 && user_page_limit < user_pages {
            user_pages = user_page_limit;
        }
        let kernel_pages = free_pages - user_pages;

        // Initialise each pool. The bitmap is stored at the beginning of
        // the pool's region, followed by the allocatable pages.
        let kernel_start = free_start as *mut u8;
        init_pool(&raw mut KERNEL_POOL, kernel_start, kernel_pages);

        let user_start = kernel_start.add(
            Bitmap::byte_cnt(kernel_pages).div_ceil(PGSIZE) * PGSIZE
                + kernel_pages * PGSIZE,
        );
        init_pool(&raw mut USER_POOL, user_start, user_pages);
    }
}

/// Set up a single pool: place the bitmap at the base, then the pages.
unsafe fn init_pool(pool: *mut Pool, start: *mut u8, page_cnt: usize) {
    let bm_bytes = Bitmap::byte_cnt(page_cnt);
    // Round bitmap size up to full pages.
    let bm_pages = bm_bytes.div_ceil(PGSIZE);

    (*pool).bitmap = Bitmap::create_in_buf(page_cnt, start, bm_pages * PGSIZE);
    (*pool).base = start.add(bm_pages * PGSIZE);
}

/// Allocate a single page.
pub fn get_page(flags: PallocFlags) -> *mut u8 {
    get_multiple(flags, 1)
}

/// Allocate `page_cnt` contiguous pages.
pub fn get_multiple(flags: PallocFlags, page_cnt: usize) -> *mut u8 {
    if page_cnt == 0 {
        return core::ptr::null_mut();
    }

    let pool = if flags.contains(PallocFlags::USER) {
        &raw mut USER_POOL
    } else {
        &raw mut KERNEL_POOL
    };

    let _guard = InterruptGuard::new();

    let page_idx = unsafe { (*pool).bitmap.scan_and_flip(0, page_cnt, false) };

    let pages = match page_idx {
        Some(idx) => unsafe { (*pool).base.add(idx * PGSIZE) },
        None => {
            if flags.contains(PallocFlags::ASSERT) {
                panic!("palloc_get: out of pages");
            }
            return core::ptr::null_mut();
        }
    };

    if flags.contains(PallocFlags::ZERO) {
        unsafe {
            core::ptr::write_bytes(pages, 0, page_cnt * PGSIZE);
        }
    }

    pages
}

/// Free a single page previously obtained from `get_page`.
#[allow(dead_code)]
pub fn free_page(page: *mut u8) {
    free_multiple(page, 1);
}

/// Free `page_cnt` contiguous pages previously obtained from `get_multiple`.
#[allow(dead_code)]
pub fn free_multiple(pages: *mut u8, page_cnt: usize) {
    if pages.is_null() || page_cnt == 0 {
        return;
    }

    let pool = unsafe {
        let addr = pages as usize;
        let kp = &raw mut KERNEL_POOL;
        // Determine which pool the pages belong to.
        if addr >= (*kp).base as usize
            && addr < (*kp).base as usize + (*kp).bitmap.size() * PGSIZE
        {
            kp
        } else {
            &raw mut USER_POOL
        }
    };

    let _guard = InterruptGuard::new();

    unsafe {
        let page_idx = (pages as usize - (*pool).base as usize) / PGSIZE;
        // Mark pages as free (false).
        (*pool).bitmap.set_multiple(page_idx, page_cnt, false);
    }
}
