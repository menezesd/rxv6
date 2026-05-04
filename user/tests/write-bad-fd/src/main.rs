#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(write-bad-fd) begin");
    let n = syscall::write(99, b"hello");
    if n == -1 {
        println!("(write-bad-fd) PASSED");
    } else {
        println!("(write-bad-fd) FAILED: got {}", n);
    }
    println!("(write-bad-fd) end");
    0
}
