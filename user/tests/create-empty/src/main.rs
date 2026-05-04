#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-empty) begin");
    let ok = syscall::create(b"\0", 0);
    if ok {
        println!("(create-empty) FAILED: create with empty name succeeded");
    } else {
        println!("(create-empty) PASSED: create with empty name failed");
    }
    println!("(create-empty) end");
    0
}
