#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-rm-root) begin");
    let ok = syscall::remove(b"/\0");
    if !ok {
        println!("(dir-rm-root) PASSED");
    } else {
        println!("(dir-rm-root) FAILED");
    }
    0
}
