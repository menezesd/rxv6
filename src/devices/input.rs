//! Console input with line editing (like xv6 consoleintr/consoleread).
//!
//! Two modes:
//! - Cooked (default): line-buffered with echo and editing (backspace, Ctrl-U).
//! - Raw: every byte is immediately available, no echo, no editing.

use crate::sync::InterruptGuard;

const BUF_SIZE: usize = 128;

const CTRL_C: u8 = 0x03;
const CTRL_D: u8 = 0x04;
const CTRL_U: u8 = 0x15;
const CTRL_Z: u8 = 0x1A;
const CTRL_BACKSLASH: u8 = 0x1C;
const BACKSPACE: u8 = 0x08;
const DEL: u8 = 0x7F;

struct InputBuffer {
    buf: [u8; BUF_SIZE],
    r: usize, // Read index  (consumer takes from here)
    w: usize, // Write index (committed; advanced on newline)
    e: usize, // Edit index  (new chars go here)
    raw: bool, // Raw mode: no editing, no echo, immediate delivery
}

static mut INPUT: InputBuffer = InputBuffer {
    buf: [0; BUF_SIZE],
    r: 0,
    w: 0,
    e: 0,
    raw: false,
};

/// Set raw mode on/off. Returns previous state.
pub fn set_raw(raw: bool) -> bool {
    let _guard = InterruptGuard::new();
    unsafe {
        let old = INPUT.raw;
        INPUT.raw = raw;
        old
    }
}

/// Get current raw mode state.
pub fn is_raw() -> bool {
    unsafe { INPUT.raw }
}

/// Erase one character on the console.
fn echo_backspace() {
    crate::kprint!("\x08 \x08");
}

/// Put a byte directly into the buffer without any processing (for escape sequences).
/// Called from interrupt context.
pub fn putc_raw(c: u8) {
    unsafe {
        if INPUT.e.wrapping_sub(INPUT.r) < BUF_SIZE {
            INPUT.buf[INPUT.e % BUF_SIZE] = c;
            INPUT.e += 1;
            if INPUT.raw {
                INPUT.w = INPUT.e;
            }
        }
    }
}

/// Called from interrupt context (keyboard / serial IRQ).
/// In cooked mode: handles line editing, echo, and \r → \n translation.
/// In raw mode: delivers bytes immediately with no processing.
pub fn putc(c: u8) {
    unsafe {
        if INPUT.raw {
            // Raw mode: no editing, no echo, immediate delivery
            if INPUT.e.wrapping_sub(INPUT.r) < BUF_SIZE {
                INPUT.buf[INPUT.e % BUF_SIZE] = c;
                INPUT.e += 1;
                INPUT.w = INPUT.e;
            }
            return;
        }

        // Cooked mode
        match c {
            CTRL_C => {
                crate::kprint!("^C\n");
                let pgid = crate::thread::foreground_pgid();
                if pgid > 0 {
                    crate::thread::send_signal_pgid(pgid, crate::thread::SIGINT);
                }
                INPUT.e = INPUT.w;
            }
            CTRL_BACKSLASH => {
                crate::kprint!("^\\\n");
                let pgid = crate::thread::foreground_pgid();
                if pgid > 0 {
                    crate::thread::send_signal_pgid(pgid, crate::thread::SIGQUIT);
                }
                INPUT.e = INPUT.w;
            }
            CTRL_Z => {
                crate::kprint!("^Z\n");
                // No job control — just echo and ignore
            }
            CTRL_U => {
                // Kill line: erase back to start of current input
                while INPUT.e != INPUT.w
                    && INPUT.buf[(INPUT.e - 1) % BUF_SIZE] != b'\n'
                {
                    INPUT.e -= 1;
                    echo_backspace();
                }
            }
            BACKSPACE | DEL => {
                if INPUT.e != INPUT.w {
                    INPUT.e -= 1;
                    echo_backspace();
                }
            }
            _ => {
                if c != 0 && INPUT.e.wrapping_sub(INPUT.r) < BUF_SIZE {
                    let c = if c == b'\r' { b'\n' } else { c };
                    INPUT.buf[INPUT.e % BUF_SIZE] = c;
                    INPUT.e += 1;
                    if c == CTRL_D {
                        crate::kprint!("^D");
                    } else {
                        crate::kprint!("{}", c as char);
                    }
                    // Commit the line on newline, Ctrl-D, or buffer full
                    if c == b'\n' || c == CTRL_D || INPUT.e == INPUT.r + BUF_SIZE {
                        INPUT.w = INPUT.e;
                    }
                }
            }
        }
    }
}

/// Get one character, blocking until data is available.
/// In cooked mode, blocks until a committed line is available.
/// In raw mode, blocks until any byte is available.
/// Returns 0 if the process was killed while waiting.
#[allow(dead_code)]
pub fn getc() -> u8 {
    loop {
        {
            let _guard = InterruptGuard::new();
            unsafe {
                if INPUT.r != INPUT.w {
                    let c = INPUT.buf[INPUT.r % BUF_SIZE];
                    INPUT.r += 1;
                    return c;
                }
            }
        }
        let t = crate::thread::running_thread();
        if unsafe { (*t).killed } {
            return 0;
        }
        crate::thread::yield_current();
    }
}

/// Returns true if the input buffer is full.
#[allow(dead_code)]
pub fn full() -> bool {
    unsafe { INPUT.e.wrapping_sub(INPUT.r) >= BUF_SIZE }
}

/// Returns true if there is committed data available to read.
pub fn has_data() -> bool {
    let _guard = InterruptGuard::new();
    unsafe { INPUT.r != INPUT.w }
}
