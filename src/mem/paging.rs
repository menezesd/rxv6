//! Kernel page-table initialisation.
//!
//! Replaces the boot-time 4 MB PSE mappings with proper 4 KB page tables.
//! All physical memory is mapped into the higher half at PHYS_BASE.
//!
//! Ported from Pintos init.c `paging_init()`.

use super::palloc::{self, PallocFlags};
use super::vaddr::*;

extern "C" {
    static _start_text: u8;
    static _end_kernel_text: u8;
}

/// The kernel's master page directory (kernel virtual address).
static mut INIT_PAGE_DIR: *mut u32 = core::ptr::null_mut();

/// Read total RAM pages (reuse palloc's detection with 64MB cap).
fn get_ram_pages() -> usize {
    crate::mem::palloc::detect_ram_pages()
}

/// Build proper 4 KB page tables and switch CR3.
pub fn init() {
    let ram_pages = get_ram_pages();

    // Allocate the page directory.
    let pd = palloc::get_page(PallocFlags::ASSERT | PallocFlags::ZERO) as *mut u32;

    let kernel_text_start = unsafe { &_start_text as *const u8 as usize };
    let kernel_text_end = unsafe { &_end_kernel_text as *const u8 as usize };

    // Current page table (only set when a new PDE is created).
    let mut pt: *mut u32 = core::ptr::null_mut();

    for page in 0..ram_pages {
        let paddr = page * PGSIZE;
        let vaddr = ptov(paddr);

        let pde_idx = vaddr >> 22;
        let pte_idx = (vaddr >> 12) & 0x3FF;

        unsafe {
            if *pd.add(pde_idx) == 0 {
                pt = palloc::get_page(PallocFlags::ASSERT | PallocFlags::ZERO) as *mut u32;
                // PDE: present + writable + user-accessible
                *pd.add(pde_idx) = vtop(pt as usize) as u32 | PTE_P | PTE_W | PTE_U;
            }

            // Kernel text/rodata is read-only; everything else is writable.
            let writable = !(vaddr >= kernel_text_start && vaddr < kernel_text_end);
            let flags = PTE_P | if writable { PTE_W } else { 0 };
            *pt.add(pte_idx) = paddr as u32 | flags;
        }
    }

    unsafe {
        INIT_PAGE_DIR = pd;

        // Switch to the new page directory.
        let pd_phys = vtop(pd as usize) as u32;
        core::arch::asm!(
            "mov cr3, {pd}",
            pd = in(reg) pd_phys,
            options(nostack, preserves_flags),
        );
    }
}

/// Return the kernel's page directory (virtual address).
#[allow(dead_code)]
pub fn page_dir() -> *mut u32 {
    unsafe { INIT_PAGE_DIR }
}
