//! PIT timer driver (Intel 8254).
//!
//! Configures the PIT to fire at TIMER_FREQ Hz and counts ticks.
//! Ported from Pintos devices/timer.c.

use crate::arch::idt::{self, IntrFrame, IntrLevel};
use crate::arch::port::Port;
use crate::sync::InterruptGuard;

/// Timer interrupt frequency (ticks per second).
pub const TIMER_FREQ: u32 = 100;

/// PIT I/O ports.
const PIT_CHANNEL0: Port = Port::new(0x40);
const PIT_COMMAND: Port = Port::new(0x43);

/// Total timer ticks since boot. Accessed only with interrupts disabled.
static mut TICKS: i64 = 0;

// ── Sleep queue ──────────────────────────────────────────────────────

struct SleepEntry {
    wakeup_tick: i64,
    thread: *mut crate::thread::Thread,
}

/// Sorted list of sleeping threads (sorted by wakeup_tick, ascending).
static mut SLEEP_LIST: Option<alloc::vec::Vec<SleepEntry>> = None;

fn init_sleep_list() {
    unsafe { SLEEP_LIST = Some(alloc::vec::Vec::new()); }
}

/// Sleep for approximately `ticks_to_sleep` timer ticks.
/// Uses a sleep queue instead of busy-waiting.
pub fn timer_sleep(ticks_to_sleep: i64) {
    if ticks_to_sleep <= 0 {
        return;
    }
    let wakeup = ticks() + ticks_to_sleep;
    let old = idt::intr_disable();
    let t = crate::thread::running_thread();
    unsafe {
        if let Some(ref mut list) = SLEEP_LIST {
            // Insert sorted by wakeup_tick
            let pos = list.iter().position(|e| e.wakeup_tick > wakeup).unwrap_or(list.len());
            list.insert(pos, SleepEntry { wakeup_tick: wakeup, thread: t });
        }
    }
    crate::thread::block();
    idt::intr_set_level(old);
}

/// Wake up any threads whose wakeup_tick has arrived.
/// Called from timer_interrupt with interrupts disabled.
fn wake_sleeping_threads() {
    let now = unsafe { TICKS };
    unsafe {
        if let Some(ref mut list) = SLEEP_LIST {
            while let Some(first) = list.first() {
                if first.wakeup_tick > now {
                    break;
                }
                let entry = list.remove(0);
                crate::thread::unblock(entry.thread);
            }
        }
    }
}

/// Initialize the PIT to fire at TIMER_FREQ and register the timer IRQ.
pub fn init() {
    // Initialize sleep list
    init_sleep_list();

    // Configure PIT channel 0 in rate generator mode (mode 2).
    // Divisor = PIT_BASE_FREQ / TIMER_FREQ.
    let divisor = (1193180 + TIMER_FREQ / 2) / TIMER_FREQ;

    // Command: channel 0, lobyte/hibyte, mode 2, binary
    PIT_COMMAND.write_u8(0x34);
    PIT_CHANNEL0.write_u8((divisor & 0xFF) as u8);
    PIT_CHANNEL0.write_u8((divisor >> 8) as u8);

    // Register timer interrupt handler (IRQ 0 = vector 0x20).
    idt::register_ext(0, timer_interrupt, "8254 Timer");

    // Unmask IRQ 0 on the PIC.
    idt::pic_unmask(0);
}

/// Return the number of ticks since boot.
pub fn ticks() -> i64 {
    let _guard = InterruptGuard::new();
    unsafe { core::ptr::read_volatile(&raw const TICKS) }
}

/// Return elapsed ticks since `then`.
#[allow(dead_code)]
pub fn elapsed(then: i64) -> i64 {
    ticks() - then
}

/// Timer interrupt handler. Called at TIMER_FREQ Hz.
fn timer_interrupt(_frame: &mut IntrFrame) {
    unsafe { TICKS += 1; }

    // Wake any sleeping threads whose time has come
    wake_sleeping_threads();

    crate::thread::tick();
}

/// Busy-wait for approximately `loops` iterations.
#[inline(never)]
fn busy_wait(loops: i64) {
    let mut i = loops;
    while i > 0 {
        core::hint::spin_loop();
        i -= 1;
    }
}

/// Calibrate loops_per_tick for busy-wait delays.
/// Must be called with interrupts enabled.
static mut LOOPS_PER_TICK: u32 = 0;

pub fn calibrate() {
    assert_eq!(idt::intr_get_level(), IntrLevel::On);
    crate::kprint!("Calibrating timer...  ");

    // Find largest power-of-2 that doesn't exceed one tick.
    let mut loops_per_tick: u32 = 1 << 10;
    while !too_many_loops(loops_per_tick << 1) {
        loops_per_tick <<= 1;
    }

    // Refine the next 8 bits.
    let high_bit = loops_per_tick;
    let mut test_bit = high_bit >> 1;
    while test_bit != high_bit >> 10 {
        if !too_many_loops(loops_per_tick | test_bit) {
            loops_per_tick |= test_bit;
        }
        test_bit >>= 1;
    }

    unsafe { LOOPS_PER_TICK = loops_per_tick; }
    crate::kprintln!("{} loops/s.", loops_per_tick as u64 * TIMER_FREQ as u64);
}

/// Returns true if `loops` iterations takes more than one tick.
fn too_many_loops(loops: u32) -> bool {
    let start = ticks();
    while ticks() == start {
        core::hint::spin_loop();
    }
    let start = ticks();
    busy_wait(loops as i64);
    start != ticks()
}
