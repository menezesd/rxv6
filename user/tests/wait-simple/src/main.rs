#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(wait-simple) begin");
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    if pid == -1 {
        println!("(wait-simple) FAILED: exec returned -1");
        return 1;
    }
    let status = syscall::wait(pid);
    if status == 81 {
        println!("(wait-simple) PASSED");
    } else {
        println!("(wait-simple) FAILED: expected 81, got {}", status);
    }
    println!("(wait-simple) end");
    0
}
