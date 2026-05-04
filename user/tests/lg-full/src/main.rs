#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"lgf.dat\0", 0);
    let fd = syscall::open(b"lgf.dat\0");
    let block = [0xCCu8; 512];
    for _ in 0..16 {
        syscall::write(fd, &block);
    }
    let sz = syscall::filesize(fd);
    if sz == 8192 {
        println!("(lg-full) PASSED");
    }
    syscall::close(fd);
    0
}
