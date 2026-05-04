#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exec-bad-ptr) begin");
    syscall::syscall1(syscall::SYS_EXEC, 0xC0000000);
    println!("(exec-bad-ptr) FAILED");
    0
}
