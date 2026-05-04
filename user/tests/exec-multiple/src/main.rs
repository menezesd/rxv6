#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exec-multiple) begin");
    for i in 0..4 {
        let pid = syscall::exec(b"child-simple\0" as *const u8);
        let status = syscall::wait(pid);
        println!("(exec-multiple) child {} exited with {}", i, status);
    }
    println!("(exec-multiple) PASSED");
    0
}
