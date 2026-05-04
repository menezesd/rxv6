#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(read-bad-fd) begin");
    let mut buf = [0u8; 8];
    let n = syscall::read(99, &mut buf);
    if n == -1 {
        println!("(read-bad-fd) PASSED");
    } else {
        println!("(read-bad-fd) FAILED: got {}", n);
    }
    println!("(read-bad-fd) end");
    0
}
