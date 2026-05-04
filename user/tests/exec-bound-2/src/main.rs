#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exec-bound-2) begin");
    let pid = syscall::exec(b"exit\0" as *const u8);
    let s = syscall::wait(pid);
    println!("(exec-bound-2) child exited with {}", s);
    if s == 42 {
        println!("(exec-bound-2) PASSED");
    }
    0
}
