#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-long) begin");
    // Create a very long filename (exceeds typical filesystem limits)
    let ok = syscall::create(b"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz\0", 0);
    if ok {
        println!("(create-long) FAILED: create with very long name succeeded");
    } else {
        println!("(create-long) PASSED: create with long name failed");
    }
    println!("(create-long) end");
    0
}
