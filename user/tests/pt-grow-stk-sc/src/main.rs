#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-grow-stk-sc) begin");
    // Make a syscall with a buffer near the stack boundary
    // This tests that the page fault handler works for stack growth
    let mut big = [0u8; 4000]; // near page boundary
    big[0] = 42;
    syscall::write(1, &big[..1]); // write one byte from stack buffer
    println!("(pt-grow-stk-sc) PASSED");
    0
}
