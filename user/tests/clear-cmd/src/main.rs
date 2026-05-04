//! clear - clear the terminal screen.

#![no_std]
#![no_main]

use rxv6_user::syscall;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    // ESC[2J = clear screen, ESC[H = cursor home
    syscall::write(1, b"\x1b[2J\x1b[H");
    0
}
