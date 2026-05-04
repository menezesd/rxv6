//! Input character buffer for keyboard/serial input.
//!
//! Ported from Pintos input.c. Uses a simple circular buffer with
//! interrupt-disable synchronization.

use crate::sync::InterruptGuard;

const BUF_SIZE: usize = 128;

struct InputBuffer {
    buf: [u8; BUF_SIZE],
    head: usize,
    tail: usize,
    count: usize,
}

static mut INPUT: InputBuffer = InputBuffer {
    buf: [0; BUF_SIZE],
    head: 0,
    tail: 0,
    count: 0,
};

/// Add a character to the input buffer.
/// Called from interrupt context (interrupts already off).
pub fn putc(c: u8) {
    unsafe {
        if INPUT.count < BUF_SIZE {
            INPUT.buf[INPUT.tail] = c;
            INPUT.tail = (INPUT.tail + 1) % BUF_SIZE;
            INPUT.count += 1;
        }
    }
}

/// Get a character from the input buffer, blocking if empty.
#[allow(dead_code)]
pub fn getc() -> u8 {
    loop {
        {
            let _guard = InterruptGuard::new();
            unsafe {
                if INPUT.count > 0 {
                    let c = INPUT.buf[INPUT.head];
                    INPUT.head = (INPUT.head + 1) % BUF_SIZE;
                    INPUT.count -= 1;
                    return c;
                }
            }
        }
        // Buffer empty - yield and retry
        crate::thread::yield_current();
    }
}

/// Returns true if the input buffer is full.
#[allow(dead_code)]
pub fn full() -> bool {
    unsafe { INPUT.count >= BUF_SIZE }
}
