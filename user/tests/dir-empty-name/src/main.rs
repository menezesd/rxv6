#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-empty-name) begin");
    let ok = syscall::mkdir(b"\0");
    if !ok {
        println!("(dir-empty-name) PASSED");
    } else {
        println!("(dir-empty-name) FAILED");
    }
    0
}
