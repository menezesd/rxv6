#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(open-twice) begin");
    syscall::create(b"twice.txt\0", 0);
    let fd1 = syscall::open(b"twice.txt\0");
    let fd2 = syscall::open(b"twice.txt\0");
    if fd1 >= 2 && fd2 >= 2 && fd1 != fd2 {
        println!("(open-twice) PASSED: fd1={} fd2={}", fd1, fd2);
    } else {
        println!("(open-twice) FAILED: fd1={} fd2={}", fd1, fd2);
    }
    syscall::close(fd1);
    syscall::close(fd2);
    println!("(open-twice) end");
    0
}
