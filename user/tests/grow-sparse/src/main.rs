#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(grow-sparse) begin");
    syscall::create(b"gs.dat\0", 0);
    let fd = syscall::open(b"gs.dat\0");
    syscall::seek(fd, 5000);
    syscall::write(fd, b"X");
    let sz = syscall::filesize(fd);
    syscall::close(fd);
    if sz == 5001 {
        println!("(grow-sparse) PASSED");
    } else {
        println!("(grow-sparse) FAILED sz={}", sz);
    }
    0
}
