#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-null) begin");
    // Pass address 0 as filename pointer via raw syscall - should kill the process
    syscall::syscall2(syscall::SYS_CREATE, 0, 0);
    println!("(create-null) FAILED: should have been killed");
    syscall::exit(-1);
}
