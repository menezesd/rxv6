#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(write-bad-ptr) begin");
    syscall::create(b"wbp.dat\0", 0);
    let fd = syscall::open(b"wbp.dat\0");
    syscall::syscall3(syscall::SYS_WRITE, fd as u32, 0xC0000000, 100);
    println!("(write-bad-ptr) FAILED");
    0
}
