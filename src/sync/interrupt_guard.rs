//! RAII guard that disables interrupts while held and restores the
//! previous interrupt state on drop.
//!
//! Uses the IDT module's interrupt level functions.

use crate::arch::idt::{IntrLevel, intr_disable, intr_set_level};

pub struct InterruptGuard {
    old_level: IntrLevel,
}

impl InterruptGuard {
    /// Disable interrupts and return a guard that restores the previous state.
    pub fn new() -> Self {
        let old = intr_disable();
        InterruptGuard { old_level: old }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        intr_set_level(self.old_level);
    }
}
