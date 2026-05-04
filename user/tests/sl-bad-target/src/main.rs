#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sl-bad-target) begin");
    let fd = syscall::open(b"noexist-link\0");
    if fd < 0 {
        println!("(sl-bad-target) PASSED");
    } else {
        syscall::close(fd);
        println!("(sl-bad-target) FAILED");
    }
    0
}
