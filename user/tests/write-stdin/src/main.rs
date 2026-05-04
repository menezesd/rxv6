#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(write-stdin) begin");
    let n = syscall::write(0, b"hello");
    if n == -1 {
        println!("(write-stdin) PASSED");
    } else {
        println!("(write-stdin) FAILED: got {}", n);
    }
    println!("(write-stdin) end");
    0
}
