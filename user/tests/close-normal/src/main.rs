#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(close-normal) begin");
    syscall::create(b"c.txt\0", 0);
    let fd = syscall::open(b"c.txt\0");
    syscall::close(fd);
    println!("(close-normal) PASSED");
    0
}
