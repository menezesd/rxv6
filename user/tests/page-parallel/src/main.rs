#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(page-parallel) begin");
    let p1 = syscall::exec(b"qsort\0" as *const u8);
    let p2 = syscall::exec(b"child-simple\0" as *const u8);
    let s1 = syscall::wait(p1);
    let s2 = syscall::wait(p2);
    if s1 == 0 && s2 == 81 {
        println!("(page-parallel) PASSED");
    } else {
        println!("(page-parallel) FAILED s1={} s2={}", s1, s2);
    }
    0
}
