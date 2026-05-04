//! Mutex lock with ownership tracking and priority donation.
//!
//! A lock is a binary semaphore with an additional constraint: only the
//! thread that acquired the lock may release it. This enables ownership
//! checks and priority donation.
//!
//! Ported from Pintos synch.c.

use core::ptr;
use crate::thread::{self, Thread};
use super::semaphore::Semaphore;

pub struct Lock {
    holder: *mut Thread,
    sema: Semaphore,
}

// Safety: only accessed with interrupts disabled internally.
unsafe impl Send for Lock {}
unsafe impl Sync for Lock {}

impl Lock {
    pub const fn new() -> Self {
        Lock {
            holder: ptr::null_mut(),
            sema: Semaphore::new(1),
        }
    }

    /// Acquire the lock, blocking until it is available.
    /// Implements priority donation: if the lock is held by a lower-priority
    /// thread, donate our priority to it.
    pub fn acquire(&mut self) {
        assert!(!self.held_by_current_thread(), "lock already held");

        let old = crate::arch::idt::intr_disable();

        // Priority donation: if lock is held, donate our priority to holder
        if !self.holder.is_null() {
            let my_pri = thread::get_priority();
            // Record that we're waiting on this lock (for nested donation)
            let cur = thread::running_thread();
            unsafe { (*cur).waiting_on_lock = self as *mut Lock; }
            thread::donate_priority(self.holder, my_pri);
        }

        crate::arch::idt::intr_set_level(old);

        self.sema.down();

        // We got the lock - clear waiting_on_lock and set holder
        let cur = thread::running_thread();
        unsafe { (*cur).waiting_on_lock = ptr::null_mut(); }
        self.holder = cur;
    }

    /// Try to acquire without blocking. Returns true on success.
    #[allow(dead_code)]
    pub fn try_acquire(&mut self) -> bool {
        if self.sema.try_down() {
            self.holder = thread::running_thread();
            true
        } else {
            false
        }
    }

    /// Release the lock. Restores priority and may yield if a higher-priority
    /// thread was waiting.
    pub fn release(&mut self) {
        assert!(self.held_by_current_thread(), "lock not held by current thread");
        // Restore original priority (remove donation for this lock)
        thread::restore_priority();
        self.holder = ptr::null_mut();
        self.sema.up();
    }

    /// Returns true if the current thread holds this lock.
    pub fn held_by_current_thread(&self) -> bool {
        self.holder == thread::running_thread()
    }

    /// Return the raw holder pointer (used for nested priority donation).
    pub fn holder_raw(&self) -> *mut Thread {
        self.holder
    }
}
