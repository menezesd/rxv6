#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(read-stdout) begin");
    let mut buf = [0u8; 8];
    let n = syscall::read(1, &mut buf);
    if n == -1 {
        println!("(read-stdout) PASSED");
    } else {
        println!("(read-stdout) FAILED: got {}", n);
    }
    println!("(read-stdout) end");
    0
}
