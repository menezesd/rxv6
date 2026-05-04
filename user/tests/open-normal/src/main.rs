#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(open-normal) begin");
    syscall::create(b"t.txt\0", 100);
    let fd = syscall::open(b"t.txt\0");
    if fd >= 2 { println!("(open-normal) PASSED fd={}", fd); } else { println!("(open-normal) FAILED fd={}", fd); }
    syscall::close(fd);
    println!("(open-normal) end");
    0
}
