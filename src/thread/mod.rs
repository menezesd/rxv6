//! Thread system for RustOS.
//!
//! Ported from Pintos threads/thread.c. Each thread occupies a single
//! 4KB page: the Thread struct lives at offset 0, and the kernel stack
//! grows downward from the top of the page.

pub mod switch;

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::ptr;

use crate::arch::idt;
use crate::mem::palloc::{self, PallocFlags};
use crate::mem::vaddr::PGSIZE;

// ── Constants ────────────────────────────────────────────────────────

/// Magic number for detecting stack overflow.
pub const THREAD_MAGIC: u32 = 0xcd6abf4b;

/// Priority bounds and default.
#[allow(dead_code)]
pub const PRI_MIN: i32 = 0;
pub const PRI_DEFAULT: i32 = 31;
#[allow(dead_code)]
pub const PRI_MAX: i32 = 63;

/// Timer ticks per time slice.
const TIME_SLICE: u32 = 4;

// ── Types ────────────────────────────────────────────────────────────

/// Thread entry point signature.
pub type ThreadFunc = fn(*mut u8);

/// Thread identifier.
pub type Tid = i32;

/// Sentinel for failed thread creation.
pub const TID_ERROR: Tid = -1;

/// Thread lifecycle state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum ThreadStatus {
    Running = 0,
    Ready = 1,
    Blocked = 2,
    Dying = 3,
}

/// Thread control block.  Lives at offset 0 of a 4KB page.
/// The kernel stack occupies the rest of the page, growing downward.
#[repr(C)]
pub struct Thread {
    pub tid: Tid,
    pub status: ThreadStatus,
    pub name: [u8; 16],
    pub stack: *mut u8,
    pub priority: i32,
    /// Base priority before any donations.
    pub original_priority: i32,
    /// Page directory for user processes (null for kernel threads).
    pub pagedir: *mut u32,
    /// File descriptor table for user processes (null for kernel threads).
    pub fd_table: *mut crate::userprog::process::FdTable,
    /// Lock this thread is currently waiting on (null if not waiting).
    pub waiting_on_lock: *mut crate::sync::lock::Lock,
    // `magic` MUST be the last field -- stack-overflow sentinel.
    pub magic: u32,
}

#[allow(dead_code)]
impl Thread {
    /// Returns the page directory as an Option (None for kernel threads).
    pub fn pagedir_opt(&self) -> Option<*mut u32> {
        if self.pagedir.is_null() { None } else { Some(self.pagedir) }
    }

    /// Returns the fd table as an Option (None for kernel threads).
    ///
    /// # Safety
    /// Caller must ensure exclusive access (e.g., interrupts disabled).
    pub unsafe fn fd_table_opt(&mut self) -> Option<&mut crate::userprog::process::FdTable> {
        if self.fd_table.is_null() {
            None
        } else {
            // Safety: if non-null, fd_table was allocated by FdTable::new() in start_process
            // and remains valid for the thread's lifetime.
            Some(&mut *self.fd_table)
        }
    }

    /// Returns the lock this thread is waiting on, if any.
    pub fn waiting_lock_opt(&self) -> Option<*mut crate::sync::lock::Lock> {
        if self.waiting_on_lock.is_null() { None } else { Some(self.waiting_on_lock) }
    }
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

// ── Stack frame layouts (must match switch.S exactly) ────────────────

/// Frame pushed/popped by `switch_threads`.
#[repr(C)]
struct SwitchThreadsFrame {
    edi: u32,
    esi: u32,
    ebp: u32,
    ebx: u32,
    eip: u32,  // return address
    cur: u32,  // first arg to switch_threads
    next: u32, // second arg to switch_threads
}

/// Sits between SwitchThreadsFrame and KernelThreadFrame.
/// `eip` is popped by `ret` in switch_entry and leads to `kernel_thread`.
#[repr(C)]
struct SwitchEntryFrame {
    eip: u32,
}

/// Topmost frame on a new thread's stack (never actually "returned to";
/// kernel_thread picks up `function` and `aux` from here).
#[repr(C)]
struct KernelThreadFrame {
    eip: u32,       // unused (NULL) -- fake return address
    function: u32,  // ThreadFunc pointer
    aux: u32,       // argument to function
}

// ── Global state ─────────────────────────────────────────────────────

static mut READY_LIST: Option<VecDeque<*mut Thread>> = None;
static mut ALL_LIST: Option<Vec<*mut Thread>> = None;
static mut IDLE_THREAD: *mut Thread = ptr::null_mut();
static mut INITIAL_THREAD: *mut Thread = ptr::null_mut();
static mut THREAD_TICKS: u32 = 0;
static mut NEXT_TID: Tid = 1;

/// Offset of the `stack` field in Thread, exported for switch.S.
#[no_mangle]
pub static thread_stack_ofs: u32 = core::mem::offset_of!(Thread, stack) as u32;

// ── Helpers ──────────────────────────────────────────────────────────

#[inline(always)]
fn ready_list() -> &'static mut VecDeque<*mut Thread> {
    static_mut!(READY_LIST)
}

#[inline(always)]
fn all_list() -> &'static mut Vec<*mut Thread> {
    static_mut!(ALL_LIST)
}


fn allocate_tid() -> Tid {
    unsafe {
        let tid = NEXT_TID;
        NEXT_TID += 1;
        tid
    }
}

// ── Core functions ───────────────────────────────────────────────────

/// Return a pointer to the running thread.
/// The Thread struct is at the base of the current 4KB stack page.
pub fn running_thread() -> *mut Thread {
    let esp: u32;
    unsafe {
        core::arch::asm!("mov {}, esp", out(reg) esp);
    }
    (esp & !0xFFF) as *mut Thread
}

/// Initialise a Thread struct at `t`.  Does NOT add to any list.
unsafe fn init_thread(t: *mut Thread, name: &str, priority: i32) {
    let t = &mut *t;
    t.status = ThreadStatus::Blocked;
    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(15);
    t.name[..len].copy_from_slice(&name_bytes[..len]);
    t.name[len] = 0;
    // Stack starts at the top of the page containing `t`.
    t.stack = ((t as *mut Thread as usize) + PGSIZE) as *mut u8;
    t.priority = priority;
    t.original_priority = priority;
    t.pagedir = core::ptr::null_mut();
    t.fd_table = core::ptr::null_mut();
    t.waiting_on_lock = core::ptr::null_mut();
    t.magic = THREAD_MAGIC;
}

/// Called once during boot (single-threaded, interrupts off).
pub fn init() {
    unsafe {
        READY_LIST = Some(VecDeque::new());
        ALL_LIST = Some(Vec::new());
    }

    // The current execution context is running on the boot stack set up
    // in start.S. Convert it into the "main" thread.
    let t = running_thread();
    unsafe {
        init_thread(t, "main", PRI_DEFAULT);
        (*t).status = ThreadStatus::Running;
        (*t).tid = allocate_tid();
        INITIAL_THREAD = t;
    }
    all_list().push(t);
}

/// Create a new kernel thread.  Returns its TID, or `TID_ERROR` on failure.
pub fn create(name: &str, priority: i32, function: ThreadFunc, aux: *mut u8) -> Tid {
    let page = palloc::get_page(PallocFlags::ZERO);
    if page.is_null() {
        return TID_ERROR;
    }

    let t = page as *mut Thread;
    unsafe {
        init_thread(t, name, priority);
        (*t).tid = allocate_tid();
    }

    // Build the initial stack frames (top of page, growing down).
    // Layout (high address → low address):
    //   KernelThreadFrame
    //   SwitchEntryFrame
    //   SwitchThreadsFrame   ← t->stack will point here
    unsafe {
        let mut sp = (page as usize + PGSIZE) as *mut u8;

        // KernelThreadFrame
        sp = sp.sub(core::mem::size_of::<KernelThreadFrame>());
        let ktf = sp as *mut KernelThreadFrame;
        (*ktf).eip = 0; // null return address
        (*ktf).function = function as *const () as u32;
        (*ktf).aux = aux as u32;

        // SwitchEntryFrame -- switch_entry's `ret` will pop this,
        // jumping to kernel_thread.
        sp = sp.sub(core::mem::size_of::<SwitchEntryFrame>());
        let sef = sp as *mut SwitchEntryFrame;
        (*sef).eip = kernel_thread as *const () as u32;

        // SwitchThreadsFrame
        sp = sp.sub(core::mem::size_of::<SwitchThreadsFrame>());
        let stf = sp as *mut SwitchThreadsFrame;
        (*stf).edi = 0;
        (*stf).esi = 0;
        (*stf).ebp = 0;
        (*stf).ebx = 0;
        (*stf).eip = switch::switch_entry as *const () as u32;
        // cur/next will be filled by the actual call to switch_threads;
        // we leave them as zero.
        (*stf).cur = 0;
        (*stf).next = 0;

        (*t).stack = sp;
    }

    let tid = unsafe { (*t).tid };

    all_list().push(t);
    unblock(t);

    tid
}

/// Transition a blocked thread to the ready state.
pub fn unblock(t: *mut Thread) {
    let old_level = idt::intr_disable();
    unsafe {
        assert_eq!((*t).status, ThreadStatus::Blocked);
        (*t).status = ThreadStatus::Ready;
    }
    ready_list().push_back(t);
    // If not in interrupt context and unblocked thread has higher priority, yield
    if !idt::intr_context() {
        let cur = running_thread();
        let cur_pri = unsafe { (*cur).priority };
        let new_pri = unsafe { (*t).priority };
        if new_pri > cur_pri {
            // Restore old level first, then yield
            idt::intr_set_level(old_level);
            yield_current();
            return;
        }
    }
    idt::intr_set_level(old_level);
}

/// Block the current thread.  The caller must arrange for it to be
/// unblocked at some point, or it will stay blocked forever.
pub fn block() {
    assert_eq!(idt::intr_get_level(), idt::IntrLevel::Off);
    unsafe {
        (*running_thread()).status = ThreadStatus::Blocked;
    }
    schedule();
}

/// Yield the CPU.  The current thread is placed back on the ready queue.
pub fn yield_current() {
    let old_level = idt::intr_disable();
    let cur = running_thread();
    unsafe {
        if cur != IDLE_THREAD {
            (*cur).status = ThreadStatus::Ready;
            ready_list().push_back(cur);
        } else {
            // Idle thread: just set status so schedule()'s assert passes.
            (*cur).status = ThreadStatus::Ready;
        }
    }
    schedule();
    idt::intr_set_level(old_level);
}

/// Terminate the current thread.  Does not return.
pub fn exit() -> ! {
    idt::intr_disable();
    let cur = running_thread();
    // Remove from the all-threads list.
    all_list().retain(|&t| t != cur);
    unsafe {
        (*cur).status = ThreadStatus::Dying;
    }
    schedule();
    unreachable!("thread_exit: should not return");
}

/// Called from timer interrupt to account for the current time slice.
pub fn tick() {
    unsafe {
        THREAD_TICKS += 1;
        if THREAD_TICKS >= TIME_SLICE {
            idt::intr_yield_on_return();
        }
    }
}

/// Return the name of the running thread as a &str.
pub fn current_name() -> &'static str {
    let t = running_thread();
    unsafe {
        let name = &(*t).name;
        let len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        core::str::from_utf8_unchecked(&name[..len])
    }
}

// ── Idle thread ──────────────────────────────────────────────────────

/// The idle thread.  Runs when no other thread is ready.
fn idle(_aux: *mut u8) {
    // Signal thread_start() that idle is running (like Pintos sema_up).
    unsafe {
        if let Some(ref mut sema) = IDLE_SEMA {
            sema.up();
        }
    }

    loop {
        idt::intr_disable();
        block();

        // Re-enable interrupts atomically with HLT so we wake on the
        // next interrupt and don't spin burn CPU.
        unsafe {
            core::arch::asm!("sti", "hlt", options(nomem, nostack));
        }
    }
}

static mut IDLE_SEMA: Option<crate::sync::Semaphore> = None;

/// Create the idle thread and enable interrupts.  Called after all
/// other initialisation is complete.
pub fn start() {
    unsafe { IDLE_SEMA = Some(crate::sync::Semaphore::new(0)); }

    let idle_tid = create("idle", PRI_MIN, idle, ptr::null_mut());
    unsafe {
        IDLE_THREAD = *all_list().iter().find(|&&t| (*t).tid == idle_tid)
            .expect("idle thread not found");
    }

    // Enable interrupts so the timer can fire.
    idt::intr_enable();

    // Wait for idle to start. We temporarily lower main's priority to
    // PRI_MIN so idle (also PRI_MIN) can be scheduled via round-robin.
    let old_pri = get_priority();
    set_priority(PRI_MIN);
    unsafe {
        if let Some(ref mut sema) = IDLE_SEMA {
            sema.down();
        }
    }
    set_priority(old_pri);
}

// ── Scheduler ────────────────────────────────────────────────────────

/// Choose the next thread to run.  Returns the idle thread if the
/// ready queue is empty.  Picks the highest-priority thread.
fn next_thread_to_run() -> *mut Thread {
    let ready = ready_list();
    if ready.is_empty() {
        return unsafe { IDLE_THREAD };
    }
    // Find index of highest priority thread
    let mut best_idx = 0;
    let mut best_pri = i32::MIN;
    for (i, &t) in ready.iter().enumerate() {
        let pri = unsafe { (*t).priority };
        if pri > best_pri {
            best_pri = pri;
            best_idx = i;
        }
    }
    ready.remove(best_idx).unwrap()
}

/// Switch threads.
fn schedule() {
    let cur = running_thread();
    let next = next_thread_to_run();

    assert_eq!(idt::intr_get_level(), idt::IntrLevel::Off);
    assert_ne!(unsafe { (*cur).status }, ThreadStatus::Running);
    check_thread(next);

    unsafe {
        THREAD_TICKS = 0;
    }

    if cur != next {
        let prev = unsafe { switch::switch_threads(cur, next) };
        schedule_tail(prev);
    } else {
        schedule_tail(ptr::null_mut());
    }
}

/// Runs after every context switch.  Must be `#[no_mangle] extern "C"`
/// because `switch_entry` in switch.S calls it directly.
#[no_mangle]
pub extern "C" fn thread_schedule_tail(prev: *mut Thread) {
    schedule_tail(prev);
}

fn schedule_tail(prev: *mut Thread) {
    let cur = running_thread();
    unsafe {
        (*cur).status = ThreadStatus::Running;
    }

    // Activate the new thread's page directory and update TSS.
    crate::userprog::process::activate();

    // If the thread we switched away from is dying, free its page.
    if !prev.is_null() {
        unsafe {
            if (*prev).status == ThreadStatus::Dying && prev != INITIAL_THREAD {
                palloc::free_page(prev as *mut u8);
            }
        }
    }
}

/// Entry point for new threads.  Called from switch_entry (via ret).
/// `extern "C"` because its address is placed on the stack and called
/// from assembly.
#[no_mangle]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn kernel_thread(function: ThreadFunc, aux: *mut u8) {
    idt::intr_enable();
    function(aux);
    exit();
}

/// Return the page directory for the thread with the given TID, or null.
pub fn get_pagedir_by_tid(tid: Tid) -> *mut u32 {
    for &t in all_list().iter() {
        if unsafe { (*t).tid } == tid {
            return unsafe { (*t).pagedir };
        }
    }
    core::ptr::null_mut()
}


/// Set the current thread's priority.
/// If we lowered our priority below a ready thread's, yield.
pub fn set_priority(new_priority: i32) {
    let t = running_thread();
    unsafe {
        (*t).priority = new_priority;
        (*t).original_priority = new_priority;
    }
    yield_if_not_highest();
}

/// Get the current thread's (effective) priority.
pub fn get_priority() -> i32 {
    let t = running_thread();
    unsafe { (*t).priority }
}

/// If any ready thread has higher priority than us, yield.
pub fn yield_if_not_highest() {
    let old = idt::intr_disable();
    let cur_pri = get_priority();
    let ready = ready_list();
    let should_yield = ready.iter().any(|&t| unsafe { (*t).priority } > cur_pri);
    idt::intr_set_level(old);
    if should_yield {
        yield_current();
    }
}

/// Donate priority to a thread (used by lock acquire for priority donation).
/// Propagates nested donation up to depth 8.
pub fn donate_priority(holder: *mut Thread, priority: i32) {
    unsafe {
        let mut target = holder;
        for _ in 0..8 {
            if target.is_null() { break; }
            if priority > (*target).priority {
                (*target).priority = priority;
                // Nested: if target is waiting on a lock, propagate to that lock's holder
                let lock = (*target).waiting_on_lock;
                if !lock.is_null() {
                    target = (*lock).holder_raw();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
}

/// Restore current thread's priority to its original (base) priority.
pub fn restore_priority() {
    let t = running_thread();
    unsafe {
        (*t).priority = (*t).original_priority;
    }
}

fn check_thread(t: *mut Thread) {
    unsafe {
        assert!(!t.is_null(), "schedule: null thread");
        assert_eq!((*t).magic, THREAD_MAGIC, "schedule: stack overflow detected");
    }
}
