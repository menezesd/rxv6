#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(open-null) begin");
    // Pass a null pointer as filename - should kill the process
    syscall::syscall1(syscall::SYS_OPEN, 0);
    println!("(open-null) FAILED: should have been killed");
    syscall::exit(-1);
}
