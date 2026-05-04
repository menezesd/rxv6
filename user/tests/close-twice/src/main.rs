#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(close-twice) begin");
    syscall::create(b"dupclose.txt\0", 0);
    let fd = syscall::open(b"dupclose.txt\0");
    if fd < 2 {
        println!("(close-twice) FAILED: could not open file");
        return 1;
    }
    syscall::close(fd);
    syscall::close(fd);
    println!("(close-twice) end");
    println!("(close-twice) PASSED");
    0
}
