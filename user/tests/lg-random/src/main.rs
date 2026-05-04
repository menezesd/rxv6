#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"lgr.dat\0", 0);
    let fd = syscall::open(b"lgr.dat\0");
    syscall::write(fd, &[0u8; 4096]);
    syscall::seek(fd, 1000);
    syscall::write(fd, b"MARKER");
    syscall::seek(fd, 1000);
    let mut buf = [0u8; 6];
    syscall::read(fd, &mut buf);
    if &buf == b"MARKER" {
        println!("(lg-random) PASSED");
    }
    syscall::close(fd);
    0
}
