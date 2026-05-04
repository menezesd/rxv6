#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(close-stdin) begin");
    syscall::close(0);
    println!("(close-stdin) end");
    println!("(close-stdin) PASSED");
    0
}
