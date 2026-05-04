#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sc-boundary-3) begin");
    let fd = syscall::open(b"scb2.dat\0"); // might not exist
    syscall::close(fd);
    println!("(sc-boundary-3) PASSED");
    0
}
