pub mod block;
pub mod ide;
pub mod input;
pub mod keyboard;
pub mod serial;
pub mod shutdown;
pub mod timer;
pub mod vga;

/// Write raw bytes to console (VGA + serial), handling \n → \r\n for serial.
pub fn console_write(buf: &[u8]) {
    for &b in buf {
        if b == b'\n' {
            serial::putc(b'\r');
        }
        serial::putc(b);
        vga::write_byte(b);
    }
}
