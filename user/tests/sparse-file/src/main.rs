#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sparse-file) begin");
    syscall::create(b"sparse.dat\0", 0);
    let fd = syscall::open(b"sparse.dat\0");
    syscall::seek(fd, 1000);
    syscall::write(fd, b"END");
    let sz = syscall::filesize(fd);
    if sz == 1003 {
        println!("(sparse-file) PASSED");
    } else {
        println!("(sparse-file) FAILED sz={}", sz);
    }
    syscall::close(fd);
    0
}
