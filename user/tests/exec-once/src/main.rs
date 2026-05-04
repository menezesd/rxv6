#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exec-once) begin");
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    if pid == -1 {
        println!("(exec-once) FAILED: exec returned -1");
    } else {
        let status = syscall::wait(pid);
        println!("(exec-once) child exited with status {}", status);
        if status == 81 {
            println!("(exec-once) PASSED");
        } else {
            println!("(exec-once) FAILED: expected 81, got {}", status);
        }
    }
    println!("(exec-once) end");
    0
}
