#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exec-bound-3) begin");
    let pid = syscall::exec(b"hello\0" as *const u8);
    let s = syscall::wait(pid);
    if s == 0 {
        println!("(exec-bound-3) PASSED");
    }
    0
}
