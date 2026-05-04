#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"smr.dat\0", 0);
    let fd = syscall::open(b"smr.dat\0");
    syscall::write(fd, &[0u8; 512]);
    // Write at random offsets
    syscall::seek(fd, 100);
    syscall::write(fd, b"ABCD");
    syscall::seek(fd, 200);
    syscall::write(fd, b"EFGH");
    // Read back
    syscall::seek(fd, 100);
    let mut buf = [0u8; 4];
    syscall::read(fd, &mut buf);
    if &buf == b"ABCD" {
        println!("(sm-random) PASSED");
    }
    syscall::close(fd);
    0
}
