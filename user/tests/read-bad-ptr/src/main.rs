#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(read-bad-ptr) begin");
    syscall::create(b"rbp.dat\0", 100);
    let fd = syscall::open(b"rbp.dat\0");
    // Pass kernel address as buffer - should be killed
    syscall::syscall3(syscall::SYS_READ, fd as u32, 0xC0000000, 100);
    println!("(read-bad-ptr) FAILED");
    0
}
