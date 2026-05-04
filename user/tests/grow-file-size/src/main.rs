#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"gfs.dat\0", 0);
    let fd = syscall::open(b"gfs.dat\0");
    syscall::write(fd, &[0u8; 100]);
    let s1 = syscall::filesize(fd);
    syscall::write(fd, &[0u8; 200]);
    let s2 = syscall::filesize(fd);
    if s1 == 100 && s2 == 300 {
        println!("(grow-file-size) PASSED");
    }
    syscall::close(fd);
    0
}
