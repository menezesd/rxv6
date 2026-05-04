#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exec-bound) begin");
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    if pid > 0 {
        let s = syscall::wait(pid);
        println!("(exec-bound) PASSED status={}", s);
    } else {
        println!("(exec-bound) FAILED");
    }
    0
}
