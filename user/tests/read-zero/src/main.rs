#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(read-zero) begin");
    syscall::create(b"rz.txt\0", 0);
    let fd = syscall::open(b"rz.txt\0");
    let mut buf = [0u8; 1];
    let n = syscall::read(fd, &mut buf[..0]);
    if n == 0 {
        println!("(read-zero) PASSED");
    } else {
        println!("(read-zero) FAILED: got {}", n);
    }
    syscall::close(fd);
    println!("(read-zero) end");
    0
}
