#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"synrm.dat\0", 0);
    let fd = syscall::open(b"synrm.dat\0");
    syscall::write(fd, b"data");
    // Remove while open
    syscall::remove(b"synrm.dat\0");
    // Should still be able to read
    syscall::seek(fd, 0);
    let mut buf = [0u8; 4];
    let n = syscall::read(fd, &mut buf);
    if n == 4 {
        println!("(syn-remove) PASSED");
    }
    syscall::close(fd);
    0
}
