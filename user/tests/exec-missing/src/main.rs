#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exec-missing) begin");
    let pid = syscall::exec(b"nonexistent\0" as *const u8);
    if pid == -1 {
        println!("(exec-missing) PASSED");
    } else {
        println!("(exec-missing) FAILED pid={}", pid);
    }
    0
}
