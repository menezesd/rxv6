#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(close-bad-fd) begin");
    syscall::close(99);
    println!("(close-bad-fd) PASSED");
    0
}
