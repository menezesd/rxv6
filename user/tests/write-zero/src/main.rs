#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(write-zero) begin");
    syscall::create(b"wz.txt\0", 0);
    let fd = syscall::open(b"wz.txt\0");
    let n = syscall::write(fd, &[]);
    if n == 0 {
        println!("(write-zero) PASSED");
    } else {
        println!("(write-zero) FAILED: got {}", n);
    }
    syscall::close(fd);
    println!("(write-zero) end");
    0
}
