#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-bound) begin");
    let ok = syscall::create(b"bound.dat\0", 512);
    if ok {
        println!("(create-bound) PASSED");
    } else {
        println!("(create-bound) FAILED");
    }
    0
}
