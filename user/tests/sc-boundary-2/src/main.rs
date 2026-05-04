#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sc-boundary-2) begin");
    let ok = syscall::create(b"scb2.dat\0", 0);
    if ok {
        println!("(sc-boundary-2) PASSED");
    }
    0
}
