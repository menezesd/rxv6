#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(read-boundary) begin");
    syscall::create(b"rb.dat\0", 0);
    let fd = syscall::open(b"rb.dat\0");
    syscall::write(fd, b"test data here!!");
    syscall::seek(fd, 0);
    let mut buf = [0u8; 16];
    let n = syscall::read(fd, &mut buf);
    if n == 16 {
        println!("(read-boundary) PASSED");
    }
    syscall::close(fd);
    0
}
