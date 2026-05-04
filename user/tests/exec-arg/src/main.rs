#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    let s = syscall::wait(pid);
    if s == 81 {
        println!("(exec-arg) PASSED");
    }
    0
}
