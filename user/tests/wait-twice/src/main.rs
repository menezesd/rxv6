#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(wait-twice) begin");
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    let s1 = syscall::wait(pid);
    let s2 = syscall::wait(pid);
    println!("(wait-twice) first wait={}", s1);
    println!("(wait-twice) second wait={}", s2);
    if s1 == 81 && s2 == -1 {
        println!("(wait-twice) PASSED");
    } else {
        println!("(wait-twice) FAILED");
    }
    0
}
