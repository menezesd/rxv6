#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"gt.dat\0", 0);
    let fd = syscall::open(b"gt.dat\0");
    let t0 = syscall::tell(fd);
    syscall::write(fd, b"hello");
    let t1 = syscall::tell(fd);
    syscall::write(fd, b"world");
    let t2 = syscall::tell(fd);
    if t0 == 0 && t1 == 5 && t2 == 10 {
        println!("(grow-tell) PASSED");
    }
    syscall::close(fd);
    0
}
