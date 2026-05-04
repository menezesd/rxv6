//! Serial port driver (16550A UART, COM1).
//!
//! Polling-mode only for Phase 1. Interrupt-driven mode will be
//! added when the IDT is set up (Phase 2).
//!
//! Adapted from Pintos devices/serial.c.

use crate::arch::port::Port;
use core::fmt;

// COM1 I/O port addresses
const IO_BASE: u16 = 0x3F8;
const THR: Port = Port::new(IO_BASE);     // Transmitter Holding Register (write)
const IER: Port = Port::new(IO_BASE + 1); // Interrupt Enable Register
const FCR: Port = Port::new(IO_BASE + 2); // FIFO Control Register (write)
const LCR: Port = Port::new(IO_BASE + 3); // Line Control Register
const MCR: Port = Port::new(IO_BASE + 4); // Modem Control Register
const LSR: Port = Port::new(IO_BASE + 5); // Line Status Register (read)
const DLL: Port = Port::new(IO_BASE);     // Divisor Latch Low (when DLAB=1)
const DLH: Port = Port::new(IO_BASE + 1); // Divisor Latch High (when DLAB=1)

// Line Control Register bits
const LCR_N81: u8 = 0x03;   // No parity, 8 data bits, 1 stop bit
const LCR_DLAB: u8 = 0x80;  // Divisor Latch Access Bit

// Modem Control Register bits
const MCR_OUT2: u8 = 0x08;  // OUT2 (enables interrupts on 16550)

// Line Status Register bits
const LSR_THRE: u8 = 0x20;  // Transmitter Holding Register Empty

/// Initialize the serial port for polling-mode output.
/// Configures COM1 to 9600 baud, N-8-1.
pub fn init() {
    IER.write_u8(0x00);          // Disable all interrupts
    FCR.write_u8(0x00);          // Disable FIFO

    // Set baud rate: 115200 / divisor. For 9600 baud, divisor = 12.
    let divisor: u16 = 12;
    LCR.write_u8(LCR_N81 | LCR_DLAB);  // Enable DLAB
    DLL.write_u8((divisor & 0xFF) as u8);
    DLH.write_u8((divisor >> 8) as u8);
    LCR.write_u8(LCR_N81);              // Disable DLAB, set N-8-1

    MCR.write_u8(MCR_OUT2);     // Enable interrupt output (needed later)
}

/// Write a single byte to the serial port, busy-waiting until ready.
pub fn putc(byte: u8) {
    // Wait for transmitter holding register to be empty
    while (LSR.read_u8() & LSR_THRE) == 0 {
        core::hint::spin_loop();
    }
    THR.write_u8(byte);
}

/// Serial port writer that implements `core::fmt::Write`.
pub struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                putc(b'\r');
            }
            putc(byte);
        }
        Ok(())
    }
}

/// Write a formatted string to the serial port.
pub fn write_fmt(args: fmt::Arguments) {
    use fmt::Write;
    SerialWriter.write_fmt(args).unwrap();
}
