#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(grow-two-files) begin");
    syscall::create(b"g1.dat\0", 0);
    syscall::create(b"g2.dat\0", 0);
    let f1 = syscall::open(b"g1.dat\0");
    let f2 = syscall::open(b"g2.dat\0");
    for _ in 0..10 {
        syscall::write(f1, b"AAAA");
        syscall::write(f2, b"BBBB");
    }
    let s1 = syscall::filesize(f1);
    let s2 = syscall::filesize(f2);
    syscall::close(f1);
    syscall::close(f2);
    if s1 == 40 && s2 == 40 {
        println!("(grow-two-files) PASSED");
    } else {
        println!("(grow-two-files) FAILED");
    }
    0
}
