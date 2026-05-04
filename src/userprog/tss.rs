//! Task State Segment (TSS) management.
//!
//! The TSS enables ring 3 -> ring 0 transitions. When user code triggers
//! an interrupt, the CPU reads ESP0/SS0 from the TSS to switch to the
//! kernel stack.

use crate::arch::gdt::{self, SEL_KDSEG, SEL_TSS};
use crate::mem::palloc::{self, PallocFlags};
use crate::mem::vaddr::PGSIZE;
use crate::thread;

/// x86 Task State Segment.
#[repr(C, packed)]
#[allow(dead_code)]
struct Tss {
    back_link: u16,
    _pad0: u16,
    esp0: u32,
    ss0: u16,
    _pad1: u16,
    esp1: u32,
    ss1: u16,
    _pad2: u16,
    esp2: u32,
    ss2: u16,
    _pad3: u16,
    cr3: u32,
    eip: u32,
    eflags: u32,
    eax: u32,
    ecx: u32,
    edx: u32,
    ebx: u32,
    esp: u32,
    ebp: u32,
    esi: u32,
    edi: u32,
    es: u16,
    _pad4: u16,
    cs: u16,
    _pad5: u16,
    ss: u16,
    _pad6: u16,
    ds: u16,
    _pad7: u16,
    fs: u16,
    _pad8: u16,
    gs: u16,
    _pad9: u16,
    ldt: u16,
    _pad10: u16,
    trace: u16,
    bitmap: u16,
}

/// Pointer to the active TSS (allocated from a kernel page).
static mut TSS_PTR: *mut Tss = core::ptr::null_mut();

/// Initialise the TSS: allocate, configure, install in GDT, and load.
pub fn init() {
    // Allocate a zeroed kernel page for the TSS.
    let tss = palloc::get_page(PallocFlags::ASSERT | PallocFlags::ZERO) as *mut Tss;
    unsafe {
        (*tss).ss0 = SEL_KDSEG;
        (*tss).bitmap = 0xDFFF;
        TSS_PTR = tss;
    }

    // Install TSS descriptor in the GDT at selector 0x28.
    let base = tss as u32;
    let limit = (core::mem::size_of::<Tss>() - 1) as u16; // 103 = 0x67
    gdt::set_tss(base, limit);

    // Load the task register with the TSS selector.
    unsafe {
        core::arch::asm!(
            "ltr {sel:x}",
            sel = in(reg) SEL_TSS,
            options(nomem, nostack),
        );
    }

    // Set esp0 to current thread's kernel stack top.
    update();
}

/// Update the TSS esp0 to point to the top of the current thread's
/// kernel stack page. Called on every context switch.
pub fn update() {
    unsafe {
        if !TSS_PTR.is_null() {
            (*TSS_PTR).esp0 = thread::running_thread() as u32 + PGSIZE as u32;
        }
    }
}
