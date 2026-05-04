#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(close-stdout) begin");
    syscall::close(1);
    println!("(close-stdout) end");
    println!("(close-stdout) PASSED");
    0
}
