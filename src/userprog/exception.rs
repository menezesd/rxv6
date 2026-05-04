//! Exception handlers for RustOS.
//!
//! Registers CPU exception handlers that kill the faulting user process
//! (or panic if the fault is in kernel mode).

use crate::arch::gdt::SEL_UCSEG;
use crate::arch::idt::{self, IntrFrame, IntrLevel};
use crate::mem::vaddr::{PGMASK, PHYS_BASE};
use crate::userprog::pagedir;
use crate::vm::page::{PageLocation, PageType};

/// Register CPU exception handlers.
pub fn init() {
    // CPU exceptions — kill process on fault
    idt::register_int(0, 0, IntrLevel::On, kill, "#DE Divide Error");
    idt::register_int(6, 0, IntrLevel::On, kill, "#UD Invalid Opcode");
    idt::register_int(13, 0, IntrLevel::On, kill, "#GP General Protection");

    // Page fault must run with interrupts OFF to preserve CR2
    idt::register_int(14, 0, IntrLevel::Off, page_fault, "#PF Page Fault");
}

/// Handle a fatal CPU exception. User-mode faults kill the process;
/// kernel-mode faults panic.
fn kill(frame: &mut IntrFrame) {
    let user = frame.cs == SEL_UCSEG;
    if user {
        crate::kprintln!(
            "Process killed: exception vec={:#x} at eip={:#x}",
            frame.vec_no,
            frame.eip
        );
        crate::userprog::process::exit_with_status(-1);
        crate::thread::exit();
    } else {
        panic!(
            "Kernel exception at eip={:#x}, vec={:#x}, error_code={:#x}",
            frame.eip, frame.vec_no, frame.error_code
        );
    }
}

/// Handle a page fault. Reads the faulting address from CR2, then
/// attempts demand paging via the SPT, stack growth, or swap-in.
/// Kills the user process (or panics for kernel faults) if unresolvable.
fn page_fault(frame: &mut IntrFrame) {
    // Read faulting address from CR2 before doing anything else.
    let fault_addr: u32;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) fault_addr);
    }

    let not_present = frame.error_code & 0x1 == 0;
    let write = frame.error_code & 0x2 != 0;
    let user = frame.error_code & 0x4 != 0;

    // Re-enable interrupts (we have saved CR2).
    idt::intr_enable();

    // --- Demand paging via SPT ---
    if user && not_present {
        let fault_page = fault_addr as usize & !PGMASK;
        let t = crate::thread::running_thread();
        let pd = unsafe { (*t).pagedir };
        let tid = unsafe { (*t).tid };
        let spt = crate::userprog::process::get_spt(tid);

        if !spt.is_null() && !pd.is_null() {
            let spt_ref = unsafe { &mut *spt };
            if let Some(entry) = spt_ref.find_mut(fault_page) {
                // Write to a read-only page — kill the process immediately.
                if write && !entry.writable {
                    crate::userprog::process::exit_with_status(-1);
                    crate::thread::exit();
                }
                if crate::vm::page::load_page(entry, pd, fault_page, tid) {
                    return; // handled
                }
            }
        }
    }

    // --- Stack growth ---
    if user && not_present {
        let fault = fault_addr as usize;
        let esp = frame.esp as usize;
        let stack_limit = PHYS_BASE - 8 * 1024 * 1024; // max 8MB stack

        // Accept faults within 32 bytes below ESP (PUSHA pushes 32 bytes).
        // Normal stack accesses (sub esp, N; mov [esp], ...) fault at or
        // above the current ESP, so 32 bytes covers every case.
        if fault >= stack_limit && fault < PHYS_BASE && fault >= esp.wrapping_sub(32) {
            let page = fault & !PGMASK;
            let t = crate::thread::running_thread();
            let pd = unsafe { (*t).pagedir };
            if !pd.is_null() {
                let tid = unsafe { (*t).tid };
                let kpage = crate::vm::frame::alloc_frame(page, tid);
                if !kpage.is_null() {
                    if pagedir::set_page(pd, page, kpage as usize, true) {
                        // Add SPT entry for the new stack page.
                        let spt = crate::userprog::process::get_spt(tid);
                        if !spt.is_null() {
                            let spt_ref = unsafe { &mut *spt };
                            spt_ref.insert(crate::vm::page::SptEntry {
                                vaddr: page,
                                location: PageLocation::InMemory,
                                page_type: PageType::Anonymous,
                                writable: true,
                                inode_sector: 0,
                                file_offset: 0,
                                read_bytes: 0,
                                page_ofs: 0,
                            });
                        }
                        return; // handled — resume execution
                    }
                    crate::vm::frame::free_frame(kpage);
                }
            }
        }
    }

    // --- Legacy swap-in (for pages without SPT entries) ---
    if user && not_present {
        let fault_page = fault_addr as usize & !PGMASK;
        let t = crate::thread::running_thread();
        let tid = unsafe { (*t).tid };
        let pd = unsafe { (*t).pagedir };

        if !pd.is_null() {
            if let Some(slot) = crate::vm::frame::get_swap_slot(tid, fault_page) {
                let kpage = crate::vm::frame::alloc_frame(fault_page, tid);
                if !kpage.is_null() {
                    crate::vm::swap::swap_in(slot, kpage);
                    if pagedir::set_page(pd, fault_page, kpage as usize, true) {
                        crate::vm::frame::remove_swap_entry(tid, fault_page);
                        return; // handled
                    }
                    crate::vm::frame::free_frame(kpage);
                }
            }
        }
    }

    if user {
        crate::kprintln!(
            "Page fault at {:#x} (user, {}, {})",
            fault_addr,
            if write { "write" } else { "read" },
            if not_present { "not present" } else { "protection" }
        );
        crate::userprog::process::exit_with_status(-1);
        crate::thread::exit();
    } else {
        panic!(
            "Kernel page fault at {:#x} (eip={:#x}, {}, {})",
            fault_addr,
            frame.eip,
            if write { "write" } else { "read" },
            if not_present { "not present" } else { "protection" }
        );
    }
}
