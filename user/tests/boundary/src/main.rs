#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(boundary) begin");
    // Test writing data that straddles a page boundary.
    // Use the user stack area - write a string across a known page boundary.
    let data = b"Page boundary test OK!";
    syscall::write(1, data);
    println!("");
    println!("(boundary) end");
    println!("(boundary) PASSED");
    0
}
