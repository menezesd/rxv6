//! echo - print arguments to stdout.

#![no_std]
#![no_main]

use rxv6_user::syscall;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    for i in 1..argc {
        if i > 1 {
            syscall::write(1, b" ");
        }
        let arg = unsafe { *argv.add(i as usize) };
        let s = unsafe { rxv6_user::cstr_to_str(arg) };
        syscall::write(1, s.as_bytes());
    }
    syscall::write(1, b"\n");
    0
}
