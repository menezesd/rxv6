#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sl-read) begin");
    syscall::create(b"sl-src\0", 0);
    let fd = syscall::open(b"sl-src\0");
    syscall::write(fd, b"symlink data");
    syscall::close(fd);
    // Re-open and read
    let fd = syscall::open(b"sl-src\0");
    let mut buf = [0u8; 12];
    let n = syscall::read(fd, &mut buf);
    syscall::close(fd);
    if n == 12 {
        println!("(sl-read) PASSED");
    } else {
        println!("(sl-read) FAILED");
    }
    0
}
