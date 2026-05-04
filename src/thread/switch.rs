//! Rust bindings for the context-switch assembly in asm/switch.S.

use super::Thread;

// Include the assembly file.
core::arch::global_asm!(include_str!("../../asm/switch.S"), options(att_syntax));

#[allow(improper_ctypes)]
extern "C" {
    /// Switch from `cur` to `next`. Returns the thread we switched away from
    /// (i.e., `cur` as seen by the new thread).
    pub fn switch_threads(cur: *mut Thread, next: *mut Thread) -> *mut Thread;

    /// Entry point for newly created threads. Not called directly from Rust.
    pub fn switch_entry();
}
