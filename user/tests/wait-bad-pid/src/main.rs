#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(wait-bad-pid) begin");
    let status = syscall::wait(9999);
    if status == -1 {
        println!("(wait-bad-pid) PASSED");
    } else {
        println!("(wait-bad-pid) FAILED: got {}", status);
    }
    println!("(wait-bad-pid) end");
    0
}
