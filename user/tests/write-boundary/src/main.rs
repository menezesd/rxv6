#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(write-boundary) begin");
    syscall::create(b"wb.dat\0", 0);
    let fd = syscall::open(b"wb.dat\0");
    let data = b"boundary write!!";
    let n = syscall::write(fd, data);
    if n == 16 {
        println!("(write-boundary) PASSED");
    }
    syscall::close(fd);
    0
}
