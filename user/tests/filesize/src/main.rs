#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(filesize) begin");
    syscall::create(b"sz.txt\0", 0);
    let fd = syscall::open(b"sz.txt\0");
    syscall::write(fd, b"0123456789");
    let sz = syscall::filesize(fd);
    if sz == 10 { println!("(filesize) PASSED"); } else { println!("(filesize) FAILED sz={}", sz); }
    syscall::close(fd);
    0
}
