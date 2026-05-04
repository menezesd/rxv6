//! User page directory management.
//!
//! Each user process gets its own page directory. The kernel portion
//! (entries for addresses >= PHYS_BASE) is shared with the kernel page
//! directory; user entries (< PHYS_BASE) are per-process.

use crate::mem::palloc::{self, PallocFlags};
use crate::mem::vaddr::*;

/// Create a new page directory with kernel mappings.
///
/// Copies all 1024 entries from the kernel's init page directory so
/// that kernel memory is accessible in every address space.
pub fn create() -> *mut u32 {
    let pd = palloc::get_page(PallocFlags::ZERO) as *mut u32;
    if pd.is_null() {
        return pd;
    }
    // Copy kernel portion from init_page_dir.
    // Safety: both pd and init_pd point to 4KB-aligned page directories
    // (1024 u32 entries = 4096 bytes). This shares kernel mappings.
    let init_pd = crate::mem::paging::page_dir();
    unsafe {
        core::ptr::copy_nonoverlapping(init_pd, pd, 1024);
    }
    pd
}

/// Destroy a page directory, freeing all user pages and page tables.
///
/// Only frees pages in the user region (below PHYS_BASE). Kernel
/// mappings are shared and must not be freed here.
#[allow(dead_code)]
pub fn destroy(pd: *mut u32) {
    if pd.is_null() {
        return;
    }
    let phys_base_pde = PHYS_BASE >> 22; // index of first kernel PDE
    // Safety: pd was allocated by create() above and has 1024 valid u32 entries.
    // We only iterate user-region PDEs (indices 0..phys_base_pde).
    unsafe {
        for i in 0..phys_base_pde {
            let pde = *pd.add(i);
            if pde & PTE_P != 0 {
                let pt = ptov((pde & PTE_ADDR) as usize) as *mut u32;
                // Free all present user pages in this page table.
                for j in 0..1024 {
                    let pte = *pt.add(j);
                    if pte & PTE_P != 0 {
                        let page = ptov((pte & PTE_ADDR) as usize) as *mut u8;
                        // Remove from frame table AND free the page
                        crate::vm::frame::free_frame(page);
                    }
                }
                // Free the page table itself (kernel pool, not in frame table).
                palloc::free_page(pt as *mut u8);
            }
        }
    }
    palloc::free_page(pd as *mut u8);
}

/// Activate a page directory by loading it into CR3.
///
/// If `pd` is null, the kernel page directory is used.
pub fn activate(pd: *mut u32) {
    let pd = if pd.is_null() {
        crate::mem::paging::page_dir()
    } else {
        pd
    };
    let phys = vtop(pd as usize) as u32;
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) phys, options(nostack));
    }
}

/// Map a user virtual page to a kernel page in the given page directory.
///
/// `upage` must be page-aligned and below PHYS_BASE.
/// `kpage` is the kernel virtual address of the physical frame.
/// Returns true on success, false if a page table could not be allocated.
#[allow(dead_code)]
pub fn set_page(pd: *mut u32, upage: usize, kpage: usize, writable: bool) -> bool {
    debug_assert!(upage < PHYS_BASE);
    debug_assert!(pg_ofs(upage) == 0);
    debug_assert!(pg_ofs(kpage) == 0);

    let pte_ptr = lookup(pd, upage, true);
    if pte_ptr.is_null() {
        return false;
    }
    let flags = PTE_P | PTE_U | if writable { PTE_W } else { 0 };
    unsafe {
        *pte_ptr = (vtop(kpage) as u32) | flags;
    }
    true
}

/// Get the kernel virtual address mapped to a user address, or null.
#[allow(dead_code)]
pub fn get_page(pd: *mut u32, uaddr: usize) -> *mut u8 {
    let pte_ptr = lookup(pd, uaddr, false);
    if pte_ptr.is_null() {
        return core::ptr::null_mut();
    }
    let pte = unsafe { *pte_ptr };
    if pte & PTE_P == 0 {
        return core::ptr::null_mut();
    }
    let page_phys = (pte & PTE_ADDR) as usize;
    (ptov(page_phys) + pg_ofs(uaddr)) as *mut u8
}

/// Clear a user page mapping (set PTE to 0).
#[allow(dead_code)]
pub fn clear_page(pd: *mut u32, upage: usize) {
    let pte_ptr = lookup(pd, upage, false);
    if !pte_ptr.is_null() {
        unsafe {
            *pte_ptr = 0;
        }
        // Invalidate TLB entry for this page.
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) upage, options(nostack));
        }
    }
}

/// Check whether the accessed bit is set for `upage` in `pd`.
#[allow(dead_code)]
pub fn is_accessed(pd: *mut u32, upage: usize) -> bool {
    let pte_ptr = lookup(pd, upage, false);
    if pte_ptr.is_null() {
        return false;
    }
    let pte = unsafe { *pte_ptr };
    pte & PTE_A != 0
}

/// Set or clear the accessed bit for `upage` in `pd`.
#[allow(dead_code)]
pub fn set_accessed(pd: *mut u32, upage: usize, val: bool) {
    let pte_ptr = lookup(pd, upage, false);
    if pte_ptr.is_null() {
        return;
    }
    unsafe {
        if val {
            *pte_ptr |= PTE_A;
        } else {
            *pte_ptr &= !PTE_A;
        }
        core::arch::asm!("invlpg [{}]", in(reg) upage, options(nostack));
    }
}

/// Check whether the dirty bit is set for `upage` in `pd`.
#[allow(dead_code)]
pub fn is_dirty(pd: *mut u32, upage: usize) -> bool {
    let pte_ptr = lookup(pd, upage, false);
    if pte_ptr.is_null() {
        return false;
    }
    let pte = unsafe { *pte_ptr };
    pte & PTE_D != 0
}

/// Look up the PTE for `vaddr` in `pd`.
///
/// If `create` is true and the page table doesn't exist, allocate one.
/// Returns a pointer to the PTE, or null on failure.
fn lookup(pd: *mut u32, vaddr: usize, create: bool) -> *mut u32 {
    let pde_idx = vaddr >> 22;
    let pte_idx = (vaddr >> 12) & 0x3FF;
    unsafe {
        let pde = pd.add(pde_idx);
        if *pde == 0 {
            if !create {
                return core::ptr::null_mut();
            }
            let pt = palloc::get_page(PallocFlags::ZERO) as *mut u32;
            if pt.is_null() {
                return core::ptr::null_mut();
            }
            *pde = (vtop(pt as usize) as u32) | PTE_P | PTE_W | PTE_U;
        }
        let pt_phys = (*pde & PTE_ADDR) as usize;
        let pt = ptov(pt_phys) as *mut u32;
        pt.add(pte_idx)
    }
}
