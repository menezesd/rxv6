#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(syn-read) begin");
    syscall::create(b"synr.dat\0", 0);
    let fd = syscall::open(b"synr.dat\0");
    syscall::write(fd, b"ABCDEFGHIJ"); // 10 bytes
    syscall::close(fd);
    let f1 = syscall::open(b"synr.dat\0");
    let f2 = syscall::open(b"synr.dat\0");
    let mut b1 = [0u8; 5];
    let mut b2 = [0u8; 5];
    syscall::read(f1, &mut b1);
    syscall::read(f2, &mut b2);
    // Both should read from start
    if &b1 == b"ABCDE" && &b2 == b"ABCDE" {
        println!("(syn-read) PASSED");
    } else {
        println!("(syn-read) FAILED");
    }
    syscall::close(f1);
    syscall::close(f2);
    0
}
