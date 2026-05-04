#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(persist-write) creating file...");
    syscall::create(b"persist.dat\0", 0);
    let fd = syscall::open(b"persist.dat\0");
    syscall::write(fd, b"PERSISTED DATA OK");
    syscall::close(fd);
    println!("(persist-write) PASSED: file created");
    0
}
