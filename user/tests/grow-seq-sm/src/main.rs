#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"gsm.dat\0", 0);
    let fd = syscall::open(b"gsm.dat\0");
    for _ in 0..10 {
        syscall::write(fd, b"data");
    }
    let sz = syscall::filesize(fd);
    if sz == 40 {
        println!("(grow-seq-sm) PASSED");
    }
    syscall::close(fd);
    0
}
