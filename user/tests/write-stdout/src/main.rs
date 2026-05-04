#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let msg = b"(write-stdout) Hello via SYS_WRITE\n";
    syscall::write(1, msg);
    println!("(write-stdout) PASSED");
    0
}
