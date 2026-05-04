#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(write-normal) begin");
    let data = b"Test write!";
    syscall::create(b"wn.txt\0", 0);
    let fd = syscall::open(b"wn.txt\0");
    let n = syscall::write(fd, data);
    if n == data.len() as i32 {
        println!("(write-normal) PASSED");
    } else {
        println!("(write-normal) FAILED: wrote {} bytes", n);
    }
    syscall::close(fd);
    println!("(write-normal) end");
    0
}
