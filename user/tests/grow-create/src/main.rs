#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"grow.dat\0", 0);
    let fd = syscall::open(b"grow.dat\0");
    let sz1 = syscall::filesize(fd);
    syscall::write(fd, b"growing!");
    let sz2 = syscall::filesize(fd);
    if sz1 == 0 && sz2 == 8 {
        println!("(grow-create) PASSED");
    }
    syscall::close(fd);
    0
}
