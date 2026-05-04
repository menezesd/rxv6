#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(open-missing) begin");
    let fd = syscall::open(b"no-such-file\0");
    if fd == -1 {
        println!("(open-missing) PASSED");
    } else {
        println!("(open-missing) FAILED: got fd {}", fd);
    }
    println!("(open-missing) end");
    0
}
