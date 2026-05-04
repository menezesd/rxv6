//! VGA text-mode driver with ANSI/VT100 escape code support.
//!
//! The VGA framebuffer is at physical 0xB8000, mapped to virtual
//! 0xC00B_8000 via the higher-half kernel mapping. Each cell is
//! 2 bytes: [character, attribute].
//!
//! Supports CSI sequences: cursor movement, erase, SGR colors.

use crate::arch::port::Port;
use core::fmt;
use core::cell::UnsafeCell;

const COL_CNT: usize = 80;
const ROW_CNT: usize = 25;
const DEFAULT_ATTR: u8 = 0x07; // light gray on black

/// Virtual address of VGA framebuffer (KERNEL_OFFSET + 0xB8000).
const VGA_BUFFER: usize = 0xC00B_8000;

/// ANSI color index → VGA color (reordered: ANSI uses RGB ordering, VGA uses BGR).
const ANSI_TO_VGA: [u8; 8] = [
    0, // 0 black
    4, // 1 red
    2, // 2 green
    6, // 3 yellow (brown)
    1, // 4 blue
    5, // 5 magenta
    3, // 6 cyan
    7, // 7 white (light gray)
];

/// Escape sequence parser state.
#[derive(Clone, Copy, PartialEq)]
enum EscState {
    Normal,
    Esc,        // Got ESC (0x1B)
    Csi,        // Got ESC[
    CsiQuestion, // Got ESC[?
}

const MAX_PARAMS: usize = 8;

/// VGA text-mode writer with escape code state machine.
struct Writer {
    col: usize,
    row: usize,
    attr: u8,
    default_attr: u8,
    // Escape sequence state
    state: EscState,
    params: [u16; MAX_PARAMS],
    param_idx: usize,
    // Saved cursor position (for ESC[s / ESC[u)
    saved_row: usize,
    saved_col: usize,
}

/// Wrapper to allow a global mutable Writer without `static mut`.
struct SyncWriter(UnsafeCell<Writer>);
unsafe impl Sync for SyncWriter {}

static WRITER: SyncWriter = SyncWriter(UnsafeCell::new(Writer {
    col: 0,
    row: 0,
    attr: DEFAULT_ATTR,
    default_attr: DEFAULT_ATTR,
    state: EscState::Normal,
    params: [0; MAX_PARAMS],
    param_idx: 0,
    saved_row: 0,
    saved_col: 0,
}));

impl Writer {
    fn write_cell(&mut self, row: usize, col: usize, ch: u8, attr: u8) {
        if row >= ROW_CNT || col >= COL_CNT {
            return;
        }
        let base = VGA_BUFFER as *mut u8;
        let offset = (row * COL_CNT + col) * 2;
        unsafe {
            base.add(offset).write_volatile(ch);
            base.add(offset + 1).write_volatile(attr);
        }
    }

    fn write_byte(&mut self, byte: u8) {
        match self.state {
            EscState::Normal => self.normal_byte(byte),
            EscState::Esc => self.esc_byte(byte),
            EscState::Csi | EscState::CsiQuestion => self.csi_byte(byte),
        }
    }

    fn normal_byte(&mut self, byte: u8) {
        match byte {
            0x1B => {
                self.state = EscState::Esc;
            }
            0x08 => {
                // Backspace
                if self.col > 0 {
                    self.col -= 1;
                    self.write_cell(self.row, self.col, b' ', self.attr);
                }
            }
            b'\n' => self.newline(),
            b'\r' => self.col = 0,
            b'\t' => {
                self.col = (self.col + 8) & !7;
                if self.col >= COL_CNT {
                    self.newline();
                }
            }
            0x07 => {} // BEL - ignore
            byte => {
                if self.col >= COL_CNT {
                    self.newline();
                }
                self.write_cell(self.row, self.col, byte, self.attr);
                self.col += 1;
            }
        }
    }

    fn esc_byte(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.state = EscState::Csi;
                self.params = [0; MAX_PARAMS];
                self.param_idx = 0;
            }
            b'c' => {
                // ESC c = full reset
                self.attr = DEFAULT_ATTR;
                self.default_attr = DEFAULT_ATTR;
                self.state = EscState::Normal;
            }
            _ => {
                // Unknown escape, ignore and return to normal
                self.state = EscState::Normal;
            }
        }
    }

    fn csi_byte(&mut self, byte: u8) {
        match byte {
            b'?' => {
                self.state = EscState::CsiQuestion;
            }
            b'0'..=b'9' => {
                // Accumulate parameter digit
                if self.param_idx < MAX_PARAMS {
                    self.params[self.param_idx] =
                        self.params[self.param_idx].saturating_mul(10)
                            .saturating_add((byte - b'0') as u16);
                }
            }
            b';' => {
                // Next parameter
                if self.param_idx < MAX_PARAMS - 1 {
                    self.param_idx += 1;
                }
            }
            // Final bytes - execute the command
            b'A' => { self.cursor_up(); self.state = EscState::Normal; }
            b'B' => { self.cursor_down(); self.state = EscState::Normal; }
            b'C' => { self.cursor_forward(); self.state = EscState::Normal; }
            b'D' => { self.cursor_back(); self.state = EscState::Normal; }
            b'H' | b'f' => { self.cursor_position(); self.state = EscState::Normal; }
            b'J' => { self.erase_display(); self.state = EscState::Normal; }
            b'K' => { self.erase_line(); self.state = EscState::Normal; }
            b'm' => { self.sgr(); self.state = EscState::Normal; }
            b's' => { self.save_cursor(); self.state = EscState::Normal; }
            b'u' => { self.restore_cursor(); self.state = EscState::Normal; }
            b'h' | b'l' => {
                // ESC[?25h / ESC[?25l = show/hide cursor (ignore for now)
                self.state = EscState::Normal;
            }
            _ => {
                // Unknown final byte, discard sequence
                self.state = EscState::Normal;
            }
        }
    }

    fn param(&self, idx: usize, default: u16) -> u16 {
        if idx <= self.param_idx && self.params[idx] != 0 {
            self.params[idx]
        } else {
            default
        }
    }

    fn cursor_up(&mut self) {
        let n = self.param(0, 1) as usize;
        self.row = self.row.saturating_sub(n);
    }

    fn cursor_down(&mut self) {
        let n = self.param(0, 1) as usize;
        self.row = (self.row + n).min(ROW_CNT - 1);
    }

    fn cursor_forward(&mut self) {
        let n = self.param(0, 1) as usize;
        self.col = (self.col + n).min(COL_CNT - 1);
    }

    fn cursor_back(&mut self) {
        let n = self.param(0, 1) as usize;
        self.col = self.col.saturating_sub(n);
    }

    fn cursor_position(&mut self) {
        let row = self.param(0, 1) as usize;
        let col = self.param(1, 1) as usize;
        // ANSI is 1-based
        self.row = row.saturating_sub(1).min(ROW_CNT - 1);
        self.col = col.saturating_sub(1).min(COL_CNT - 1);
    }

    fn erase_display(&mut self) {
        match self.param(0, 0) {
            0 => {
                // Clear from cursor to end of screen
                self.erase_line_from(self.row, self.col, COL_CNT);
                for r in (self.row + 1)..ROW_CNT {
                    self.clear_row(r);
                }
            }
            1 => {
                // Clear from start to cursor
                for r in 0..self.row {
                    self.clear_row(r);
                }
                self.erase_line_from(self.row, 0, self.col + 1);
            }
            2 | 3 => {
                // Clear entire screen
                for r in 0..ROW_CNT {
                    self.clear_row(r);
                }
                self.row = 0;
                self.col = 0;
            }
            _ => {}
        }
    }

    fn erase_line(&mut self) {
        match self.param(0, 0) {
            0 => self.erase_line_from(self.row, self.col, COL_CNT),
            1 => self.erase_line_from(self.row, 0, self.col + 1),
            2 => self.clear_row(self.row),
            _ => {}
        }
    }

    fn erase_line_from(&mut self, row: usize, start: usize, end: usize) {
        for x in start..end.min(COL_CNT) {
            self.write_cell(row, x, b' ', self.attr);
        }
    }

    fn sgr(&mut self) {
        // Process all parameters (ESC[0;1;32m etc.)
        let count = self.param_idx + 1;
        let mut i = 0;
        // If no params, treat as reset
        if count == 1 && self.params[0] == 0 && self.param_idx == 0 {
            self.attr = self.default_attr;
            return;
        }
        while i < count {
            let code = self.params[i];
            match code {
                0 => self.attr = self.default_attr,
                1 => self.attr |= 0x08,  // Bold = bright foreground
                2 => self.attr &= !0x08, // Dim = remove bright
                4 => {}                   // Underline: no VGA support, ignore
                7 => {
                    // Reverse video
                    let fg = self.attr & 0x0F;
                    let bg = (self.attr >> 4) & 0x07;
                    self.attr = (fg << 4) | bg;
                }
                22 => self.attr &= !0x08, // Normal intensity
                27 => {
                    // Reverse off - reset to default
                    self.attr = self.default_attr;
                }
                // Foreground colors 30-37
                30..=37 => {
                    let color = ANSI_TO_VGA[(code - 30) as usize];
                    self.attr = (self.attr & 0xF8) | color;
                }
                // Default foreground
                39 => {
                    self.attr = (self.attr & 0xF8) | (self.default_attr & 0x07);
                }
                // Background colors 40-47
                40..=47 => {
                    let color = ANSI_TO_VGA[(code - 40) as usize];
                    self.attr = (self.attr & 0x8F) | (color << 4);
                }
                // Default background
                49 => {
                    self.attr = (self.attr & 0x8F) | (self.default_attr & 0x70);
                }
                // Bright foreground 90-97
                90..=97 => {
                    let color = ANSI_TO_VGA[(code - 90) as usize] | 0x08;
                    self.attr = (self.attr & 0xF0) | color;
                }
                // Bright background 100-107
                100..=107 => {
                    let color = ANSI_TO_VGA[(code - 100) as usize];
                    self.attr = (self.attr & 0x0F) | (color << 4) | 0x80;
                }
                _ => {} // Unknown SGR, ignore
            }
            i += 1;
        }
    }

    fn save_cursor(&mut self) {
        self.saved_row = self.row;
        self.saved_col = self.col;
    }

    fn restore_cursor(&mut self) {
        self.row = self.saved_row;
        self.col = self.saved_col;
    }

    fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= ROW_CNT {
            self.scroll();
            self.row = ROW_CNT - 1;
        }
        self.clear_row(self.row);
    }

    fn scroll(&mut self) {
        let base = VGA_BUFFER as *mut u8;
        let row_bytes = COL_CNT * 2;
        unsafe {
            core::ptr::copy(
                base.add(row_bytes),
                base,
                row_bytes * (ROW_CNT - 1),
            );
        }
    }

    fn clear_row(&mut self, row: usize) {
        for x in 0..COL_CNT {
            self.write_cell(row, x, b' ', self.attr);
        }
    }

    fn move_cursor(&self) {
        let pos = (self.row * COL_CNT + self.col) as u16;
        let idx_port = Port::new(0x3D4);
        let data_port = Port::new(0x3D5);
        idx_port.write_u8(0x0E);
        data_port.write_u8((pos >> 8) as u8);
        idx_port.write_u8(0x0F);
        data_port.write_u8(pos as u8);
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        self.move_cursor();
        Ok(())
    }
}

fn with_writer<F, R>(f: F) -> R
where
    F: FnOnce(&mut Writer) -> R,
{
    unsafe { f(&mut *WRITER.0.get()) }
}

/// Clear the screen and reset cursor to top-left.
pub fn clear() {
    with_writer(|w| {
        w.attr = DEFAULT_ATTR;
        w.col = 0;
        w.row = 0;
        for row in 0..ROW_CNT {
            w.clear_row(row);
        }
        w.move_cursor();
    });
}

/// Write a formatted string to the VGA display.
pub fn write_fmt(args: fmt::Arguments) {
    with_writer(|w| {
        fmt::Write::write_fmt(w, args).unwrap();
    });
}

/// Write a single byte to the VGA display (handles escape codes).
pub fn write_byte(byte: u8) {
    with_writer(|w| {
        w.write_byte(byte);
        w.move_cursor();
    });
}
