//! Counting semaphore.
//!
//! A semaphore is a non-negative integer with two atomic operations:
//! - `down` ("P"): wait until value > 0, then decrement
//! - `up` ("V"): increment value, wake one waiter
//!
//! Ported from Pintos synch.c.

use alloc::collections::VecDeque;
use crate::arch::idt;
use crate::thread;

pub struct Semaphore {
    value: u32,
    pub waiters: VecDeque<*mut thread::Thread>,
}

// Safety: only accessed with interrupts disabled.
unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

impl Semaphore {
    pub const fn new(value: u32) -> Self {
        Semaphore {
            value,
            waiters: VecDeque::new(),
        }
    }

    /// Decrement the semaphore, blocking if it is zero.
    /// Must not be called from an interrupt context.
    pub fn down(&mut self) {
        let old = idt::intr_disable();
        while self.value == 0 {
            self.waiters.push_back(thread::running_thread());
            thread::block();
        }
        self.value -= 1;
        idt::intr_set_level(old);
    }

    /// Try to decrement without blocking. Returns true on success.
    #[allow(dead_code)]
    pub fn try_down(&mut self) -> bool {
        let old = idt::intr_disable();
        let success = if self.value > 0 {
            self.value -= 1;
            true
        } else {
            false
        };
        idt::intr_set_level(old);
        success
    }

    /// Increment the semaphore, waking the highest-priority waiter if any.
    pub fn up(&mut self) {
        let old = idt::intr_disable();
        if !self.waiters.is_empty() {
            // Find highest-priority waiter
            let mut best_idx = 0;
            let mut best_pri = i32::MIN;
            for (i, &t) in self.waiters.iter().enumerate() {
                let pri = unsafe { (*t).priority };
                if pri > best_pri {
                    best_pri = pri;
                    best_idx = i;
                }
            }
            let waiter = self.waiters.remove(best_idx).unwrap();
            thread::unblock(waiter);
        }
        self.value += 1;
        idt::intr_set_level(old);
    }
}
