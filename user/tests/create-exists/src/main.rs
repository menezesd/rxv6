#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-exists) begin");
    let ok1 = syscall::create(b"exists.txt\0", 0);
    if !ok1 {
        println!("(create-exists) FAILED: first create failed");
        return 1;
    }
    let ok2 = syscall::create(b"exists.txt\0", 0);
    if ok2 {
        println!("(create-exists) FAILED: second create succeeded (expected failure)");
    } else {
        println!("(create-exists) PASSED: second create correctly failed");
    }
    println!("(create-exists) end");
    0
}
