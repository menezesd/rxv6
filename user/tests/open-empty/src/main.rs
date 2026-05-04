#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(open-empty) begin");
    let fd = syscall::open(b"\0");
    if fd == -1 {
        println!("(open-empty) PASSED");
    } else {
        println!("(open-empty) FAILED: got fd {}", fd);
    }
    println!("(open-empty) end");
    0
}
