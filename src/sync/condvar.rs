//! Condition variable.
//!
//! A condition variable allows threads to wait for a condition to become
//! true. Threads wait by calling `wait()` (which releases the associated
//! lock and blocks), and are woken by `signal()` or `broadcast()`.
//!
//! Ported from Pintos synch.c. Like Pintos, each waiter's semaphore lives
//! on the waiting thread's kernel stack; the condvar only stores pointers.

use alloc::collections::VecDeque;
use super::lock::Lock;
use super::semaphore::Semaphore;

pub struct Condvar {
    /// Pointers to stack-allocated semaphores owned by waiting threads.
    waiters: VecDeque<*mut Semaphore>,
}

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

#[allow(dead_code)]
impl Condvar {
    pub const fn new() -> Self {
        Condvar {
            waiters: VecDeque::new(),
        }
    }

    /// Atomically release `lock` and wait for a signal, then re-acquire `lock`.
    /// The caller must hold `lock`.
    pub fn wait(&mut self, lock: &mut Lock) {
        assert!(lock.held_by_current_thread());

        // Heap-allocate the semaphore to ensure stable address.
        let mut waiter = alloc::boxed::Box::new(Semaphore::new(0));
        let waiter_ptr = &mut *waiter as *mut Semaphore;
        self.waiters.push_back(waiter_ptr);

        lock.release();
        waiter.down(); // blocks until signal() calls up()
        lock.acquire();
        // waiter is dropped here (Box deallocated)
    }

    pub fn waiter_count(&self) -> usize { self.waiters.len() }

    /// Wake the highest-priority waiting thread. Caller must hold the associated lock.
    pub fn signal(&mut self, lock: &Lock) {
        assert!(lock.held_by_current_thread());
        if self.waiters.is_empty() { return; }

        // Find the waiter with the highest-priority blocked thread.
        // Each condvar waiter is a Semaphore* on a blocked thread's stack.
        // The semaphore has exactly one thread in its waiters VecDeque.
        let len = self.waiters.len();
        if len == 1 {
            let waiter = self.waiters.pop_front().unwrap();
            unsafe { (*waiter).up(); }
            return;
        }

        // Multiple waiters: find highest priority
        let mut best_idx = 0;
        let mut best_pri = i32::MIN;
        for i in 0..len {
            let sema_ptr = self.waiters[i];
            unsafe {
                let sema = &*sema_ptr;
                // The thread blocked on this sema
                if let Some(&t) = sema.waiters.front() {
                    let pri = (*t).priority;
                    if pri > best_pri {
                        best_pri = pri;
                        best_idx = i;
                    }
                }
            }
        }
        let waiter = self.waiters.remove(best_idx).unwrap();
        unsafe { (*waiter).up(); }
    }

    /// Wake all waiting threads. Caller must hold the associated lock.
    #[allow(dead_code)]
    pub fn broadcast(&mut self, lock: &Lock) {
        assert!(lock.held_by_current_thread());
        while let Some(waiter) = self.waiters.pop_front() {
            unsafe { (*waiter).up(); }
        }
    }
}
