#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(open-bad-ptr) begin");
    syscall::syscall1(syscall::SYS_OPEN, 0xC0000000);
    println!("(open-bad-ptr) FAILED");
    0
}
