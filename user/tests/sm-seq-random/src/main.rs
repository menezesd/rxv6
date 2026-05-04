#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sm-seq-random) begin");
    syscall::create(b"ssr.dat\0", 0);
    let fd = syscall::open(b"ssr.dat\0");
    // Write sequentially
    for i in 0u8..10 {
        syscall::write(fd, &[i; 50]);
    } // 500 bytes
    // Read randomly
    syscall::seek(fd, 150); // should be in chunk i=3
    let mut buf = [0u8; 1];
    syscall::read(fd, &mut buf);
    if buf[0] == 3 {
        println!("(sm-seq-random) PASSED");
    } else {
        println!("(sm-seq-random) FAILED");
    }
    syscall::close(fd);
    0
}
