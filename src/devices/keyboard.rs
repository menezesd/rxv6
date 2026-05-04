//! PS/2 keyboard driver.
//!
//! Ported from Pintos kbd.c. Handles scan code set 1 (default for PS/2),
//! tracks modifier state, and maps scancodes to ASCII.
//! Extended keys (arrows, Home, End, etc.) emit ANSI escape sequences.

use crate::arch::idt::{self, IntrFrame};
use crate::arch::port::Port;
use crate::devices::input;

const DATA_PORT: Port = Port::new(0x60);

// Modifier state (only accessed from interrupt handler, so safe)
static mut LEFT_SHIFT: bool = false;
static mut RIGHT_SHIFT: bool = false;
static mut LEFT_CTRL: bool = false;
static mut RIGHT_CTRL: bool = false;
static mut CAPS_LOCK: bool = false;
static mut E0_PENDING: bool = false;

/// Scancode-to-ASCII mapping for scan code set 1 (US QWERTY).
/// Index = scancode, value = (unshifted, shifted).
/// 0 means no printable character.
static SCANCODE_MAP: [(u8, u8); 128] = {
    let mut map = [(0u8, 0u8); 128];

    // Row 1: number row
    map[0x02] = (b'1', b'!');
    map[0x03] = (b'2', b'@');
    map[0x04] = (b'3', b'#');
    map[0x05] = (b'4', b'$');
    map[0x06] = (b'5', b'%');
    map[0x07] = (b'6', b'^');
    map[0x08] = (b'7', b'&');
    map[0x09] = (b'8', b'*');
    map[0x0A] = (b'9', b'(');
    map[0x0B] = (b'0', b')');
    map[0x0C] = (b'-', b'_');
    map[0x0D] = (b'=', b'+');
    map[0x0E] = (0x08, 0x08); // Backspace
    map[0x0F] = (b'\t', b'\t'); // Tab

    // Row 2: QWERTY
    map[0x10] = (b'q', b'Q');
    map[0x11] = (b'w', b'W');
    map[0x12] = (b'e', b'E');
    map[0x13] = (b'r', b'R');
    map[0x14] = (b't', b'T');
    map[0x15] = (b'y', b'Y');
    map[0x16] = (b'u', b'U');
    map[0x17] = (b'i', b'I');
    map[0x18] = (b'o', b'O');
    map[0x19] = (b'p', b'P');
    map[0x1A] = (b'[', b'{');
    map[0x1B] = (b']', b'}');
    map[0x1C] = (b'\r', b'\r'); // Enter

    // Row 3: ASDF
    map[0x1E] = (b'a', b'A');
    map[0x1F] = (b's', b'S');
    map[0x20] = (b'd', b'D');
    map[0x21] = (b'f', b'F');
    map[0x22] = (b'g', b'G');
    map[0x23] = (b'h', b'H');
    map[0x24] = (b'j', b'J');
    map[0x25] = (b'k', b'K');
    map[0x26] = (b'l', b'L');
    map[0x27] = (b';', b':');
    map[0x28] = (b'\'', b'"');
    map[0x29] = (b'`', b'~');

    // Row 4: ZXCV
    map[0x2B] = (b'\\', b'|');
    map[0x2C] = (b'z', b'Z');
    map[0x2D] = (b'x', b'X');
    map[0x2E] = (b'c', b'C');
    map[0x2F] = (b'v', b'V');
    map[0x30] = (b'b', b'B');
    map[0x31] = (b'n', b'N');
    map[0x32] = (b'm', b'M');
    map[0x33] = (b',', b'<');
    map[0x34] = (b'.', b'>');
    map[0x35] = (b'/', b'?');

    // Space
    map[0x39] = (b' ', b' ');

    // Escape
    map[0x01] = (0x1B, 0x1B);

    map
};

// Scan codes for modifier keys
const SC_LEFT_SHIFT: u8 = 0x2A;
const SC_RIGHT_SHIFT: u8 = 0x36;
const SC_LEFT_CTRL: u8 = 0x1D;
const SC_CAPS_LOCK: u8 = 0x3A;

// Extended (E0) scan codes
const E0_UP: u8 = 0x48;
const E0_DOWN: u8 = 0x50;
const E0_LEFT: u8 = 0x4B;
const E0_RIGHT: u8 = 0x4D;
const E0_HOME: u8 = 0x47;
const E0_END: u8 = 0x4F;
const E0_PGUP: u8 = 0x49;
const E0_PGDN: u8 = 0x51;
const E0_DELETE: u8 = 0x53;
const E0_INSERT: u8 = 0x52;

/// Initialize the keyboard driver.
pub fn init() {
    idt::register_ext(1, keyboard_interrupt, "keyboard");
    idt::pic_unmask(1);
    crate::kprintln!("Keyboard initialized (IRQ 1).");
}

/// Send an ANSI escape sequence into the input buffer.
fn emit_escape(seq: &[u8]) {
    for &b in seq {
        input::putc_raw(b);
    }
}

/// Keyboard interrupt handler.
fn keyboard_interrupt(_frame: &mut IntrFrame) {
    let code = DATA_PORT.read_u8();

    unsafe {
        // Handle extended scancode prefix
        if code == 0xE0 {
            E0_PENDING = true;
            return;
        }

        let is_release = code & 0x80 != 0;
        let scancode = code & 0x7F;
        let was_e0 = E0_PENDING;
        E0_PENDING = false;

        // Handle modifier keys
        if was_e0 {
            // E0 1D = right ctrl
            if scancode == SC_LEFT_CTRL {
                RIGHT_CTRL = !is_release;
                return;
            }
        } else {
            match scancode {
                SC_LEFT_SHIFT => {
                    LEFT_SHIFT = !is_release;
                    return;
                }
                SC_RIGHT_SHIFT => {
                    RIGHT_SHIFT = !is_release;
                    return;
                }
                SC_LEFT_CTRL => {
                    LEFT_CTRL = !is_release;
                    return;
                }
                SC_CAPS_LOCK if !is_release => {
                    CAPS_LOCK = !CAPS_LOCK;
                    return;
                }
                _ => {}
            }
        }

        // Ignore key releases for character keys
        if is_release {
            return;
        }

        // Handle extended keys (arrows, home, end, etc.)
        if was_e0 {
            match scancode {
                E0_UP     => emit_escape(b"\x1B[A"),
                E0_DOWN   => emit_escape(b"\x1B[B"),
                E0_RIGHT  => emit_escape(b"\x1B[C"),
                E0_LEFT   => emit_escape(b"\x1B[D"),
                E0_HOME   => emit_escape(b"\x1B[H"),
                E0_END    => emit_escape(b"\x1B[F"),
                E0_PGUP   => emit_escape(b"\x1B[5~"),
                E0_PGDN   => emit_escape(b"\x1B[6~"),
                E0_DELETE => emit_escape(b"\x1B[3~"),
                E0_INSERT => emit_escape(b"\x1B[2~"),
                _ => {}
            }
            return;
        }

        // Look up the scancode in the map
        if scancode < 128 {
            let (unshifted, shifted) = SCANCODE_MAP[scancode as usize];
            if unshifted == 0 {
                return; // No mapping for this scancode
            }

            let shift = LEFT_SHIFT || RIGHT_SHIFT;
            let ctrl = LEFT_CTRL || RIGHT_CTRL;

            // Ctrl+key: emit control character
            if ctrl {
                let base = unshifted.to_ascii_lowercase();
                if base >= b'a' && base <= b'z' {
                    input::putc(base - b'a' + 1); // Ctrl-A=1, Ctrl-C=3, etc.
                    return;
                }
            }

            // Determine if we should use shifted variant
            // Caps lock only affects letters (a-z)
            let is_letter = unshifted.is_ascii_lowercase();
            let use_shifted = if is_letter {
                shift ^ CAPS_LOCK
            } else {
                shift
            };

            let ch = if use_shifted { shifted } else { unshifted };
            input::putc(ch);
        }
    }
}
