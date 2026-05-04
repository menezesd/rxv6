#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"smf.dat\0", 0);
    let fd = syscall::open(b"smf.dat\0");
    let data = [0x42u8; 512];
    let n = syscall::write(fd, &data);
    if n == 512 {
        println!("(sm-full) PASSED");
    }
    syscall::close(fd);
    0
}
