//! VGA text-mode driver.
//!
//! The VGA framebuffer is at physical 0xB8000, mapped to virtual
//! 0xC00B_8000 via the higher-half kernel mapping. Each cell is
//! 2 bytes: [character, attribute].
//!
//! Adapted from Pintos devices/vga.c.

use crate::arch::port::Port;
use core::fmt;
use core::cell::UnsafeCell;

const COL_CNT: usize = 80;
const ROW_CNT: usize = 25;
const GRAY_ON_BLACK: u8 = 0x07;

/// Virtual address of VGA framebuffer (KERNEL_OFFSET + 0xB8000).
const VGA_BUFFER: usize = 0xC00B_8000;

/// VGA text-mode writer. Tracks cursor position and writes to the framebuffer.
struct Writer {
    col: usize,
    row: usize,
}

/// Wrapper to allow a global mutable Writer without `static mut`.
struct SyncWriter(UnsafeCell<Writer>);
unsafe impl Sync for SyncWriter {}

static WRITER: SyncWriter = SyncWriter(UnsafeCell::new(Writer { col: 0, row: 0 }));

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
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.col = 0,
            b'\t' => {
                self.col = (self.col + 8) & !7;
                if self.col >= COL_CNT {
                    self.newline();
                }
            }
            byte => {
                if self.col >= COL_CNT {
                    self.newline();
                }
                self.write_cell(self.row, self.col, byte, GRAY_ON_BLACK);
                self.col += 1;
            }
        }
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
            self.write_cell(row, x, b' ', GRAY_ON_BLACK);
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
    // Safety: single-threaded kernel with interrupts disabled during output.
    // When we add threading (Phase 3), this will use InterruptGuard.
    unsafe { f(&mut *WRITER.0.get()) }
}

/// Clear the screen and reset cursor to top-left.
pub fn clear() {
    with_writer(|w| {
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
