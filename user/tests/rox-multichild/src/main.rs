#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(rox-multichild) begin");
    let p1 = syscall::exec(b"child-simple\0" as *const u8);
    let p2 = syscall::exec(b"child-simple\0" as *const u8);
    let s1 = syscall::wait(p1);
    let s2 = syscall::wait(p2);
    if s1 == 81 && s2 == 81 {
        println!("(rox-multichild) PASSED");
    }
    0
}
