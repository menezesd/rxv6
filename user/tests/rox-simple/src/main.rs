#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let fd = syscall::open(b"rox-simple\0");
    if fd >= 0 {
        let n = syscall::write(fd, b"HACK");
        if n == 0 {
            println!("(rox-simple) PASSED: write denied");
        } else {
            println!("(rox-simple) FAILED: wrote {} bytes", n);
        }
        syscall::close(fd);
    } else {
        println!("(rox-simple) FAILED: can't open self");
    }
    0
}
