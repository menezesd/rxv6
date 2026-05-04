//! Kernel console output.
//!
//! Provides `kprint!` and `kprintln!` macros that write to both
//! the VGA text display and the serial port (for QEMU debugging).

use crate::devices;
use core::fmt;

/// Write formatted output to both VGA and serial.
pub fn write_fmt(args: fmt::Arguments) {
    devices::vga::write_fmt(args);
    devices::serial::write_fmt(args);
}

/// Print to the kernel console (VGA + serial).
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        $crate::console::write_fmt(format_args!($($arg)*))
    };
}

/// Print to the kernel console with a trailing newline.
#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => {
        $crate::kprint!("{}\n", format_args!($($arg)*))
    };
}
