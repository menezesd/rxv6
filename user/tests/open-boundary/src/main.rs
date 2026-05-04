#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(open-boundary) begin");
    syscall::create(b"ob.dat\0", 0);
    let fd = syscall::open(b"ob.dat\0");
    if fd >= 0 {
        println!("(open-boundary) PASSED");
        syscall::close(fd);
    }
    0
}
