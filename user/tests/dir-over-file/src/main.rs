#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-over-file) begin");
    syscall::create(b"afile\0", 0);
    let ok = syscall::mkdir(b"/afile\0");
    if !ok {
        println!("(dir-over-file) PASSED");
    } else {
        println!("(dir-over-file) FAILED");
    }
    0
}
