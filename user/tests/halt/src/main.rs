#![no_std]
#![no_main]

use rustos_user::syscall;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::halt();
}
