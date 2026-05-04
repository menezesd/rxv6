#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(child-sort) begin");
    let pid = syscall::exec(b"qsort\0" as *const u8);
    let s = syscall::wait(pid);
    if s == 0 {
        println!("(child-sort) PASSED");
    } else {
        println!("(child-sort) FAILED status={}", s);
    }
    0
}
