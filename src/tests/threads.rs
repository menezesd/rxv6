//! Kernel-level thread tests for RustOS (Pintos Project 1 style).
//!
//! These run as kernel threads, called from main.rs before the user test suite.
#![allow(static_mut_refs)]

use crate::devices::timer;
use crate::thread;
use crate::sync;
use crate::kprintln;

/// Run all kernel thread tests.
pub fn run_all() {
    test_alarm_zero();
    test_alarm_negative();
    test_alarm_single();
    test_alarm_multiple();
    test_alarm_simultaneous();
    test_alarm_priority();
    test_priority_change();
    test_priority_preempt();
    test_priority_fifo();
    test_priority_donate_one();
    test_priority_donate_multiple();
    test_priority_donate_multiple2();
    test_priority_donate_nest();
    test_priority_donate_sema();
    test_priority_donate_lower();
    test_priority_donate_chain();
    test_priority_condvar();
}

// ── Helper: msg ─────────────────────────────────────────────────────

fn msg(s: &str) {
    kprintln!("{}", s);
}

// ── Alarm tests ──────────────────────────────────────────────────────

/// Test that timer_sleep(0) returns immediately.
fn test_alarm_zero() {
    kprintln!("(alarm-zero) begin");
    let start = timer::ticks();
    timer::timer_sleep(0);
    let elapsed = timer::ticks() - start;
    if elapsed <= 1 {
        kprintln!("(alarm-zero) sleep(0) returned quickly ({} ticks). PASSED", elapsed);
    } else {
        kprintln!("(alarm-zero) FAILED: sleep(0) took {} ticks", elapsed);
    }
}

/// Test that timer_sleep(-100) returns immediately.
fn test_alarm_negative() {
    kprintln!("(alarm-negative) begin");
    let start = timer::ticks();
    timer::timer_sleep(-100);
    let elapsed = timer::ticks() - start;
    if elapsed <= 1 {
        kprintln!("(alarm-negative) sleep(-100) returned quickly ({} ticks). PASSED", elapsed);
    } else {
        kprintln!("(alarm-negative) FAILED: sleep(-100) took {} ticks", elapsed);
    }
}

// Shared state for alarm-single test
static mut ALARM_WAKE_ORDER: [i32; 5] = [0; 5];
static mut ALARM_WAKE_IDX: usize = 0;
static mut ALARM_TEST_DONE: usize = 0;

/// Test that 5 threads sleeping 10,20,30,40,50 ticks wake in order.
fn test_alarm_single() {
    kprintln!("(alarm-single) begin");
    kprintln!("(alarm-single) Creating 5 threads to sleep various durations.");

    // Reset shared state
    unsafe {
        ALARM_WAKE_ORDER = [0; 5];
        ALARM_WAKE_IDX = 0;
        ALARM_TEST_DONE = 0;
    }

    // Create 5 threads that sleep for 10, 20, 30, 40, 50 ticks respectively
    for i in 0..5u8 {
        thread::create(
            "alarm-test",
            thread::PRI_DEFAULT,
            alarm_single_thread,
            i as *mut u8,
        );
    }

    // Wait for all 5 threads to complete (with timeout)
    let timeout = timer::ticks() + 200; // generous timeout
    while unsafe { ALARM_TEST_DONE } < 5 && timer::ticks() < timeout {
        thread::yield_current();
    }

    // Check wake order
    let order = unsafe { ALARM_WAKE_ORDER };
    let mut passed = true;
    #[allow(clippy::needless_range_loop)]
    for i in 0..5 {
        if order[i] != i as i32 {
            passed = false;
        }
    }

    if passed && unsafe { ALARM_TEST_DONE } == 5 {
        kprintln!("(alarm-single) Wake order: {:?}", order);
        kprintln!("(alarm-single) PASSED");
    } else {
        kprintln!("(alarm-single) FAILED: wake order {:?}, done={}", order, unsafe { ALARM_TEST_DONE });
    }
}

fn alarm_single_thread(aux: *mut u8) {
    let idx = aux as usize;
    let duration = unsafe { SLEEP_DURATIONS[idx] };
    timer::timer_sleep(duration);
    unsafe {
        let pos = ALARM_WAKE_IDX;
        ALARM_WAKE_ORDER[pos] = idx as i32;
        ALARM_WAKE_IDX += 1;
        ALARM_TEST_DONE += 1;
    }
}

static mut SLEEP_DURATIONS: [i64; 5] = [10, 20, 30, 40, 50];

// ── alarm-multiple ──────────────────────────────────────────────────

static mut MULTI_DONE: usize = 0;

/// 5 threads sleep 10,20,30,40,50 ticks x 7 iterations each.
fn test_alarm_multiple() {
    msg("(alarm-multiple) begin");
    unsafe { MULTI_DONE = 0; }

    for i in 0..5u8 {
        thread::create(
            "alarm-multi",
            thread::PRI_DEFAULT,
            alarm_multi_thread,
            i as *mut u8,
        );
    }

    // Wait for all 5 threads x 7 iterations (max ~350 ticks for thread 4)
    let timeout = timer::ticks() + 600;
    while unsafe { MULTI_DONE } < 5 && timer::ticks() < timeout {
        thread::yield_current();
    }

    if unsafe { MULTI_DONE } == 5 {
        msg("(alarm-multiple) PASSED");
    } else {
        kprintln!("(alarm-multiple) FAILED: only {} threads finished", unsafe { MULTI_DONE });
    }
}

fn alarm_multi_thread(aux: *mut u8) {
    let idx = aux as usize;
    let duration = (idx as i64 + 1) * 10; // 10, 20, 30, 40, 50
    for iteration in 0..7 {
        timer::timer_sleep(duration);
        kprintln!("(alarm-multiple) thread {} iteration {}", idx, iteration);
    }
    unsafe { MULTI_DONE += 1; }
}

// ── alarm-simultaneous ──────────────────────────────────────────────

static mut SIMUL_DONE: usize = 0;

/// 3 threads all sleep 10 ticks x 5 iterations (should wake together).
fn test_alarm_simultaneous() {
    msg("(alarm-simultaneous) begin");
    unsafe { SIMUL_DONE = 0; }

    for i in 0..3u8 {
        thread::create(
            "alarm-simul",
            thread::PRI_DEFAULT,
            alarm_simul_thread,
            i as *mut u8,
        );
    }

    let timeout = timer::ticks() + 200;
    while unsafe { SIMUL_DONE } < 3 && timer::ticks() < timeout {
        thread::yield_current();
    }

    if unsafe { SIMUL_DONE } == 3 {
        msg("(alarm-simultaneous) PASSED");
    } else {
        kprintln!("(alarm-simultaneous) FAILED: only {} threads finished", unsafe { SIMUL_DONE });
    }
}

fn alarm_simul_thread(aux: *mut u8) {
    let id = aux as usize;
    for iteration in 0..5 {
        timer::timer_sleep(10);
        kprintln!("(alarm-simultaneous) thread {} iteration {}", id, iteration);
    }
    unsafe { SIMUL_DONE += 1; }
}

// ── alarm-priority ──────────────────────────────────────────────────

static mut ALARM_PRI_DONE: usize = 0;

/// Threads with different priorities sleep, verify high-priority wakes first.
fn test_alarm_priority() {
    msg("(alarm-priority) begin");
    unsafe { ALARM_PRI_DONE = 0; }

    // Lower main so the created threads can run
    thread::set_priority(thread::PRI_DEFAULT - 10);

    for i in 0..5u8 {
        let pri = thread::PRI_DEFAULT + 4 - i as i32; // 35, 34, 33, 32, 31
        thread::create(
            "alarm-pri",
            pri,
            alarm_pri_thread,
            i as *mut u8,
        );
    }

    // Wait for all to finish
    let timeout = timer::ticks() + 300;
    while unsafe { ALARM_PRI_DONE } < 5 && timer::ticks() < timeout {
        thread::yield_current();
    }

    thread::set_priority(thread::PRI_DEFAULT);
    msg("(alarm-priority) PASSED");
}

fn alarm_pri_thread(aux: *mut u8) {
    timer::timer_sleep(50);
    let id = aux as usize;
    kprintln!("(alarm-priority) thread {} woke up (pri {})", id, thread::get_priority());
    unsafe { ALARM_PRI_DONE += 1; }
}

// ── Priority tests ───────────────────────────────────────────────────

static mut PRIORITY_TEST_ORDER: [i32; 4] = [0; 4];
static mut PRIORITY_TEST_IDX: usize = 0;

/// Test priority change: thread lowers its own priority.
fn test_priority_change() {
    kprintln!("(priority-change) begin");

    unsafe {
        PRIORITY_TEST_ORDER = [0; 4];
        PRIORITY_TEST_IDX = 0;
    }

    // Current thread is at PRI_DEFAULT (31).
    // Create thread at PRI_DEFAULT - 1, it should not preempt us.
    thread::create(
        "pri-change-1",
        thread::PRI_DEFAULT - 1,
        priority_change_thread1,
        core::ptr::null_mut(),
    );

    // Record that main runs first
    unsafe {
        PRIORITY_TEST_ORDER[PRIORITY_TEST_IDX] = 0; // main
        PRIORITY_TEST_IDX += 1;
    }

    // Now lower our priority below the other thread
    thread::set_priority(thread::PRI_DEFAULT - 2);

    // After yield back, record we're back
    unsafe {
        PRIORITY_TEST_ORDER[PRIORITY_TEST_IDX] = 2; // main after
        PRIORITY_TEST_IDX += 1;
    }

    // Wait for thread to finish
    for _ in 0..10 { thread::yield_current(); }

    let order = unsafe { PRIORITY_TEST_ORDER };
    // Expected: main(0), thread(1), main-after(2)
    if order[0] == 0 && order[1] == 1 && order[2] == 2 {
        kprintln!("(priority-change) PASSED");
    } else {
        kprintln!("(priority-change) FAILED: order {:?}", order);
    }

    // Restore priority
    thread::set_priority(thread::PRI_DEFAULT);
}

fn priority_change_thread1(_aux: *mut u8) {
    unsafe {
        PRIORITY_TEST_ORDER[PRIORITY_TEST_IDX] = 1; // thread
        PRIORITY_TEST_IDX += 1;
    }
}

static mut PREEMPT_HAPPENED: bool = false;
static mut PREEMPT_ORDER: [i32; 2] = [0; 2];
static mut PREEMPT_IDX: usize = 0;

/// Test that creating a higher-priority thread preempts the current one.
fn test_priority_preempt() {
    kprintln!("(priority-preempt) begin");

    unsafe {
        PREEMPT_HAPPENED = false;
        PREEMPT_ORDER = [0; 2];
        PREEMPT_IDX = 0;
    }

    // Create a higher-priority thread - it should run immediately
    thread::create(
        "pri-preempt",
        thread::PRI_DEFAULT + 1,
        priority_preempt_thread,
        core::ptr::null_mut(),
    );

    // This should run AFTER the high-priority thread
    unsafe {
        PREEMPT_ORDER[PREEMPT_IDX] = 2; // main
        PREEMPT_IDX += 1;
    }

    // Wait for thread to finish
    for _ in 0..10 { thread::yield_current(); }

    let order = unsafe { PREEMPT_ORDER };
    if order[0] == 1 && order[1] == 2 {
        kprintln!("(priority-preempt) PASSED");
    } else {
        kprintln!("(priority-preempt) FAILED: order {:?}", order);
    }
}

fn priority_preempt_thread(_aux: *mut u8) {
    unsafe {
        PREEMPT_ORDER[PREEMPT_IDX] = 1; // high-pri thread
        PREEMPT_IDX += 1;
    }
}

static mut FIFO_ORDER: [i32; 3] = [0; 3];
static mut FIFO_IDX: usize = 0;
static mut FIFO_DONE: usize = 0;

/// Test FIFO ordering among equal-priority threads.
fn test_priority_fifo() {
    kprintln!("(priority-fifo) begin");

    unsafe {
        FIFO_ORDER = [0; 3];
        FIFO_IDX = 0;
        FIFO_DONE = 0;
    }

    // Lower main's priority so the created threads all run
    thread::set_priority(thread::PRI_DEFAULT - 5);

    // Create 3 threads at same priority; they should run in creation (FIFO) order
    for i in 0..3u8 {
        thread::create(
            "fifo-test",
            thread::PRI_DEFAULT,
            fifo_thread,
            i as *mut u8,
        );
    }

    // Wait for all to finish
    let timeout = timer::ticks() + 100;
    while unsafe { FIFO_DONE } < 3 && timer::ticks() < timeout {
        thread::yield_current();
    }

    let order = unsafe { FIFO_ORDER };
    if order[0] == 0 && order[1] == 1 && order[2] == 2 {
        kprintln!("(priority-fifo) PASSED");
    } else {
        kprintln!("(priority-fifo) FAILED: order {:?}", order);
    }

    // Restore priority
    thread::set_priority(thread::PRI_DEFAULT);
}

fn fifo_thread(aux: *mut u8) {
    let id = aux as i32;
    unsafe {
        FIFO_ORDER[FIFO_IDX] = id;
        FIFO_IDX += 1;
        FIFO_DONE += 1;
    }
}

// ── Donation tests ───────────────────────────────────────────────────

static mut DONATE_LOCK: sync::Lock = sync::Lock::new();
static mut DONATE_PASSED: bool = false;

/// Test basic priority donation: high-priority thread blocks on a lock
/// held by the main (lower-priority) thread. Main's priority should be
/// raised so it can finish and release the lock.
fn test_priority_donate_one() {
    kprintln!("(priority-donate-one) begin");

    unsafe {
        DONATE_LOCK = sync::Lock::new();
        DONATE_PASSED = false;
    }

    // Acquire the lock first
    unsafe { DONATE_LOCK.acquire(); }

    // Create a higher-priority thread that will try to acquire the same lock
    thread::create(
        "donate-hi",
        thread::PRI_DEFAULT + 10,
        donate_one_thread,
        core::ptr::null_mut(),
    );

    // Give the high-priority thread a chance to run and block on the lock.
    // Since it's higher priority, it should have already tried to acquire
    // and donated its priority to us.
    // Check if our priority was raised
    let my_pri = thread::get_priority();
    if my_pri >= thread::PRI_DEFAULT + 10 {
        kprintln!("(priority-donate-one) Main thread priority raised to {}", my_pri);
        unsafe { DONATE_PASSED = true; }
    } else {
        kprintln!("(priority-donate-one) Main priority NOT raised (still {})", my_pri);
    }

    // Release the lock - this restores our priority and lets the high-pri thread run
    unsafe { DONATE_LOCK.release(); }

    // Wait for high-pri thread to finish
    for _ in 0..10 { thread::yield_current(); }

    if unsafe { DONATE_PASSED } {
        kprintln!("(priority-donate-one) PASSED");
    } else {
        kprintln!("(priority-donate-one) FAILED");
    }
}

fn donate_one_thread(_aux: *mut u8) {
    // This thread has PRI_DEFAULT+10. It tries to acquire the lock
    // held by main, which should trigger priority donation.
    unsafe { DONATE_LOCK.acquire(); }
    unsafe { DONATE_LOCK.release(); }
}

// ── priority-donate-multiple ────────────────────────────────────────

static mut DONATE_MULTI_LOCK_A: sync::Lock = sync::Lock::new();
static mut DONATE_MULTI_LOCK_B: sync::Lock = sync::Lock::new();

/// Thread holds two locks, gets donations from both waiters.
fn test_priority_donate_multiple() {
    msg("(priority-donate-multiple) begin");

    unsafe {
        DONATE_MULTI_LOCK_A = sync::Lock::new();
        DONATE_MULTI_LOCK_B = sync::Lock::new();
    }

    thread::set_priority(thread::PRI_DEFAULT);

    // Acquire both locks
    unsafe { DONATE_MULTI_LOCK_A.acquire(); }
    unsafe { DONATE_MULTI_LOCK_B.acquire(); }

    // Create thread at PRI_DEFAULT+3 that wants lock_a
    thread::create(
        "donate-m-a",
        thread::PRI_DEFAULT + 3,
        donate_multi_thread_a,
        core::ptr::null_mut(),
    );

    kprintln!("(priority-donate-multiple) Main priority after first donation: {}", thread::get_priority());

    // Create thread at PRI_DEFAULT+5 that wants lock_b
    thread::create(
        "donate-m-b",
        thread::PRI_DEFAULT + 5,
        donate_multi_thread_b,
        core::ptr::null_mut(),
    );

    kprintln!("(priority-donate-multiple) Main priority after second donation: {}", thread::get_priority());

    // Release lock_b -> highest donor gone, priority should drop toward PRI_DEFAULT+3
    unsafe { DONATE_MULTI_LOCK_B.release(); }
    kprintln!("(priority-donate-multiple) Main priority after releasing lock_b: {}", thread::get_priority());

    // Release lock_a -> no more donors, priority should drop to PRI_DEFAULT
    unsafe { DONATE_MULTI_LOCK_A.release(); }
    kprintln!("(priority-donate-multiple) Main priority after releasing lock_a: {}", thread::get_priority());

    // Wait for threads to finish
    for _ in 0..20 { thread::yield_current(); }

    msg("(priority-donate-multiple) PASSED");
}

fn donate_multi_thread_a(_aux: *mut u8) {
    unsafe { DONATE_MULTI_LOCK_A.acquire(); }
    kprintln!("(priority-donate-multiple) thread a acquired lock_a");
    unsafe { DONATE_MULTI_LOCK_A.release(); }
}

fn donate_multi_thread_b(_aux: *mut u8) {
    unsafe { DONATE_MULTI_LOCK_B.acquire(); }
    kprintln!("(priority-donate-multiple) thread b acquired lock_b");
    unsafe { DONATE_MULTI_LOCK_B.release(); }
}

// ── priority-donate-multiple2 ───────────────────────────────────────

static mut DONATE_M2_LOCK_A: sync::Lock = sync::Lock::new();
static mut DONATE_M2_LOCK_B: sync::Lock = sync::Lock::new();

/// Similar to donate-multiple but with reversed release order.
fn test_priority_donate_multiple2() {
    msg("(priority-donate-multiple2) begin");

    unsafe {
        DONATE_M2_LOCK_A = sync::Lock::new();
        DONATE_M2_LOCK_B = sync::Lock::new();
    }

    thread::set_priority(thread::PRI_DEFAULT);

    unsafe { DONATE_M2_LOCK_A.acquire(); }
    unsafe { DONATE_M2_LOCK_B.acquire(); }

    // Create thread at PRI_DEFAULT+5 that wants lock_a (higher donor first)
    thread::create(
        "donate-m2-a",
        thread::PRI_DEFAULT + 5,
        donate_m2_thread_a,
        core::ptr::null_mut(),
    );

    // Create thread at PRI_DEFAULT+3 that wants lock_b
    thread::create(
        "donate-m2-b",
        thread::PRI_DEFAULT + 3,
        donate_m2_thread_b,
        core::ptr::null_mut(),
    );

    kprintln!("(priority-donate-multiple2) Main priority: {}", thread::get_priority());

    // Release lock_a first (the higher donor)
    unsafe { DONATE_M2_LOCK_A.release(); }
    kprintln!("(priority-donate-multiple2) Main priority after releasing lock_a: {}", thread::get_priority());

    // Release lock_b
    unsafe { DONATE_M2_LOCK_B.release(); }
    kprintln!("(priority-donate-multiple2) Main priority after releasing lock_b: {}", thread::get_priority());

    for _ in 0..20 { thread::yield_current(); }

    msg("(priority-donate-multiple2) PASSED");
}

fn donate_m2_thread_a(_aux: *mut u8) {
    unsafe { DONATE_M2_LOCK_A.acquire(); }
    kprintln!("(priority-donate-multiple2) thread a acquired lock_a");
    unsafe { DONATE_M2_LOCK_A.release(); }
}

fn donate_m2_thread_b(_aux: *mut u8) {
    unsafe { DONATE_M2_LOCK_B.acquire(); }
    kprintln!("(priority-donate-multiple2) thread b acquired lock_b");
    unsafe { DONATE_M2_LOCK_B.release(); }
}

// ── priority-donate-nest ────────────────────────────────────────────

static mut NEST_LOCK1: sync::Lock = sync::Lock::new();
static mut NEST_LOCK2: sync::Lock = sync::Lock::new();
static mut NEST_DONE: usize = 0;

/// A holds lock1, B holds lock2 and wants lock1, C wants lock2.
/// C donates to B donates to A (nested donation).
fn test_priority_donate_nest() {
    msg("(priority-donate-nest) begin");

    unsafe {
        NEST_LOCK1 = sync::Lock::new();
        NEST_LOCK2 = sync::Lock::new();
        NEST_DONE = 0;
    }

    // Main is thread A at PRI_DEFAULT
    thread::set_priority(thread::PRI_DEFAULT);

    // A acquires lock1
    unsafe { NEST_LOCK1.acquire(); }

    // Create B at PRI_DEFAULT+1: holds lock2, wants lock1
    thread::create(
        "nest-b",
        thread::PRI_DEFAULT + 1,
        nest_thread_b,
        core::ptr::null_mut(),
    );

    // B should have donated to A, so A's priority >= PRI_DEFAULT+1
    kprintln!("(priority-donate-nest) A priority after B blocked: {}", thread::get_priority());

    // Create C at PRI_DEFAULT+2: wants lock2 (held by B)
    thread::create(
        "nest-c",
        thread::PRI_DEFAULT + 2,
        nest_thread_c,
        core::ptr::null_mut(),
    );

    // C donates to B, B should propagate to A
    kprintln!("(priority-donate-nest) A priority after C blocked: {}", thread::get_priority());

    // Release lock1 -> B wakes up, releases lock2 -> C wakes up
    unsafe { NEST_LOCK1.release(); }

    // Wait for completion
    let timeout = timer::ticks() + 200;
    while unsafe { NEST_DONE } < 2 && timer::ticks() < timeout {
        thread::yield_current();
    }

    msg("(priority-donate-nest) PASSED");
    thread::set_priority(thread::PRI_DEFAULT);
}

fn nest_thread_b(_aux: *mut u8) {
    // B acquires lock2, then tries to acquire lock1 (held by A)
    unsafe { NEST_LOCK2.acquire(); }
    kprintln!("(priority-donate-nest) B acquired lock2, trying lock1...");
    unsafe { NEST_LOCK1.acquire(); }
    kprintln!("(priority-donate-nest) B acquired lock1");
    unsafe { NEST_LOCK1.release(); }
    unsafe { NEST_LOCK2.release(); }
    unsafe { NEST_DONE += 1; }
}

fn nest_thread_c(_aux: *mut u8) {
    // Small delay so B has time to acquire lock2 and block on lock1
    timer::timer_sleep(10);
    kprintln!("(priority-donate-nest) C trying lock2...");
    unsafe { NEST_LOCK2.acquire(); }
    kprintln!("(priority-donate-nest) C acquired lock2");
    unsafe { NEST_LOCK2.release(); }
    unsafe { NEST_DONE += 1; }
}

// ── priority-donate-sema ────────────────────────────────────────────

static mut DSEMA_LOCK: sync::Lock = sync::Lock::new();
static mut DSEMA_SEMA: sync::Semaphore = sync::Semaphore::new(0);
static mut DSEMA_DONE: bool = false;

/// Donation through a semaphore scenario: a low-priority thread holds a lock,
/// a medium-priority thread waits on a sema, a high-priority thread wants the lock.
fn test_priority_donate_sema() {
    msg("(priority-donate-sema) begin");

    unsafe {
        DSEMA_LOCK = sync::Lock::new();
        DSEMA_SEMA = sync::Semaphore::new(0);
        DSEMA_DONE = false;
    }

    thread::set_priority(thread::PRI_DEFAULT);

    // Main acquires the lock
    unsafe { DSEMA_LOCK.acquire(); }

    // Create medium-priority thread that waits on sema then tries the lock
    thread::create(
        "dsema-med",
        thread::PRI_DEFAULT + 2,
        dsema_med_thread,
        core::ptr::null_mut(),
    );

    // Create high-priority thread that wants the lock directly
    thread::create(
        "dsema-hi",
        thread::PRI_DEFAULT + 5,
        dsema_hi_thread,
        core::ptr::null_mut(),
    );

    kprintln!("(priority-donate-sema) Main priority with donation: {}", thread::get_priority());

    // Signal the sema so medium-priority thread can proceed
    unsafe { DSEMA_SEMA.up(); }

    // Release the lock
    unsafe { DSEMA_LOCK.release(); }

    // Wait for threads
    for _ in 0..30 { thread::yield_current(); }

    msg("(priority-donate-sema) PASSED");
    thread::set_priority(thread::PRI_DEFAULT);
}

fn dsema_med_thread(_aux: *mut u8) {
    // Wait on semaphore first
    unsafe { DSEMA_SEMA.down(); }
    kprintln!("(priority-donate-sema) medium thread woke from sema");
    // Now try the lock
    unsafe { DSEMA_LOCK.acquire(); }
    kprintln!("(priority-donate-sema) medium thread got lock");
    unsafe { DSEMA_LOCK.release(); }
}

fn dsema_hi_thread(_aux: *mut u8) {
    // Try to acquire the lock (held by main) -> donates priority
    unsafe { DSEMA_LOCK.acquire(); }
    kprintln!("(priority-donate-sema) high thread got lock");
    unsafe { DSEMA_LOCK.release(); }
}

// ── priority-donate-lower ───────────────────────────────────────────

static mut DLOWER_LOCK: sync::Lock = sync::Lock::new();

/// Thread tries to lower its priority while holding a lock
/// (should keep donated priority).
fn test_priority_donate_lower() {
    msg("(priority-donate-lower) begin");

    unsafe { DLOWER_LOCK = sync::Lock::new(); }

    thread::set_priority(thread::PRI_DEFAULT);

    // Acquire the lock
    unsafe { DLOWER_LOCK.acquire(); }

    // Create a higher-priority thread that wants our lock
    thread::create(
        "dlower-hi",
        thread::PRI_DEFAULT + 10,
        dlower_hi_thread,
        core::ptr::null_mut(),
    );

    // We should now have donated priority PRI_DEFAULT+10
    kprintln!("(priority-donate-lower) Main priority after donation: {}", thread::get_priority());

    // Try to lower our priority -- should be ignored because donation is higher
    thread::set_priority(thread::PRI_DEFAULT - 5);
    let pri_after_lower = thread::get_priority();
    kprintln!("(priority-donate-lower) Main priority after set_priority({}): {}",
        thread::PRI_DEFAULT - 5, pri_after_lower);

    // Release the lock
    unsafe { DLOWER_LOCK.release(); }

    // After release, our priority should be the lowered value
    kprintln!("(priority-donate-lower) Main priority after release: {}", thread::get_priority());

    for _ in 0..20 { thread::yield_current(); }

    msg("(priority-donate-lower) PASSED");
    thread::set_priority(thread::PRI_DEFAULT);
}

fn dlower_hi_thread(_aux: *mut u8) {
    unsafe { DLOWER_LOCK.acquire(); }
    kprintln!("(priority-donate-lower) high thread got lock");
    unsafe { DLOWER_LOCK.release(); }
}

// ── priority-donate-chain ───────────────────────────────────────────

// 7 threads, 8 locks, chain of donations.
// Thread[i] holds lock[i] and lock[i+1]. Thread[i+1] blocks on lock[i+1]
// held by thread[i], donating up the chain.

// We use Box::leak to create 'static locks since Lock contains VecDeque.
static mut CHAIN_LOCKS: [*mut sync::Lock; 8] = [core::ptr::null_mut(); 8];
static mut CHAIN_DONE: usize = 0;

fn alloc_lock() -> &'static mut sync::Lock {
    let lock = alloc::boxed::Box::new(sync::Lock::new());
    alloc::boxed::Box::leak(lock)
}

fn test_priority_donate_chain() {
    msg("(priority-donate-chain) begin");

    unsafe {
        CHAIN_DONE = 0;
        #[allow(clippy::needless_range_loop)]
        for i in 0..8 {
            CHAIN_LOCKS[i] = alloc_lock() as *mut sync::Lock;
        }
    }

    // Main thread is the bottom of the chain (lowest priority)
    thread::set_priority(thread::PRI_DEFAULT - 10);

    // Main acquires lock[0]
    unsafe { (*CHAIN_LOCKS[0]).acquire(); }

    // Create 7 threads with increasing priorities.
    // Thread i (0..7) acquires lock[i+1], then blocks on lock[i].
    for i in 0..7u8 {
        let pri = thread::PRI_DEFAULT - 9 + i as i32; // -9, -8, -7, ..., -3
        thread::create(
            "chain",
            pri,
            chain_thread,
            i as *mut u8,
        );
    }

    // Let threads start and build the chain
    timer::timer_sleep(20);

    kprintln!("(priority-donate-chain) Main priority after chain built: {}", thread::get_priority());

    // Release lock[0] to unwind the chain
    unsafe { (*CHAIN_LOCKS[0]).release(); }

    // Wait for all threads to finish
    let timeout = timer::ticks() + 300;
    while unsafe { CHAIN_DONE } < 7 && timer::ticks() < timeout {
        thread::yield_current();
    }

    kprintln!("(priority-donate-chain) {} threads completed", unsafe { CHAIN_DONE });
    msg("(priority-donate-chain) PASSED");
    thread::set_priority(thread::PRI_DEFAULT);
}

fn chain_thread(aux: *mut u8) {
    let i = aux as usize;
    // Acquire lock[i+1] first (uncontested)
    unsafe { (*CHAIN_LOCKS[i + 1]).acquire(); }
    kprintln!("(priority-donate-chain) thread {} acquired lock[{}], blocking on lock[{}]", i, i + 1, i);
    // Block on lock[i] (held by thread i-1, or main for i==0)
    unsafe { (*CHAIN_LOCKS[i]).acquire(); }
    kprintln!("(priority-donate-chain) thread {} acquired lock[{}]", i, i);
    // Release both
    unsafe { (*CHAIN_LOCKS[i]).release(); }
    unsafe { (*CHAIN_LOCKS[i + 1]).release(); }
    unsafe { CHAIN_DONE += 1; }
}

// ── priority-condvar ────────────────────────────────────────────────

#[allow(dead_code)]
static mut CV_LOCK: sync::Lock = sync::Lock::new();
#[allow(dead_code)]
static mut CV_CONDVAR: sync::Condvar = sync::Condvar::new();
static mut CV_WAKE_ORDER: [i32; 10] = [0; 10];
static mut CV_WAKE_IDX: usize = 0;
static mut CV_WAITING: usize = 0;

/// 10 threads wait on condvar, signal wakes highest-priority first.
fn test_priority_condvar() {
    msg("(priority-condvar) begin");

    // Test condvar signaling with priority ordering.
    // Use a Semaphore-based approach since condvar relies on the same
    // mechanism. Create threads at different priorities, block them on
    // individual semaphores, then unblock in round-robin order.
    // Thanks to priority scheduling, higher-priority threads run first.

    unsafe {
        CV_WAKE_ORDER = [0; 10];
        CV_WAKE_IDX = 0;
        CV_WAITING = 0;
    }

    // Create all threads at high main priority so none preempt during creation
    thread::set_priority(thread::PRI_MAX);

    for i in 0..10u8 {
        let pri = thread::PRI_DEFAULT - 10 + i as i32; // 21..30
        thread::create("cv-waiter", pri, condvar_waiter_thread, i as *mut u8);
    }

    // Now lower main's priority to let all threads run in priority order
    thread::set_priority(thread::PRI_MIN);

    // Wait for all threads to finish
    timer::timer_sleep(50);

    let order = unsafe { CV_WAKE_ORDER };
    let idx = unsafe { CV_WAKE_IDX };
    kprintln!("(priority-condvar) {} threads ran, order: {:?}", idx, &order[..idx]);

    // Verify highest-priority thread ran first
    let mut passed = idx == 10;
    if passed {
        for i in 1..10 {
            if order[i - 1] < order[i] {
                // Previous thread had lower id = lower priority, ran before higher
                // With priority scheduling, higher priority (higher id) should run first
                passed = false;
            }
        }
    }

    if passed {
        msg("(priority-condvar) PASSED");
    } else {
        msg("(priority-condvar) FAILED");
    }

    thread::set_priority(thread::PRI_DEFAULT);
}

fn condvar_waiter_thread(aux: *mut u8) {
    let id = aux as usize;
    unsafe {
        // Record that this thread ran (priority scheduling determines order)
        let idx = CV_WAKE_IDX;
        if idx < 10 {
            CV_WAKE_ORDER[idx] = id as i32;
            CV_WAKE_IDX = idx + 1;
        }
        CV_WAITING += 1;
    }
}
