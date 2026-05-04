#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(lg-seq-random) begin");
    syscall::create(b"lsr.dat\0", 0);
    let fd = syscall::open(b"lsr.dat\0");
    let block = [0xDDu8; 512];
    for _ in 0..16 {
        syscall::write(fd, &block);
    } // 8KB
    syscall::seek(fd, 4096);
    let mut buf = [0u8; 1];
    syscall::read(fd, &mut buf);
    if buf[0] == 0xDD {
        println!("(lg-seq-random) PASSED");
    } else {
        println!("(lg-seq-random) FAILED");
    }
    syscall::close(fd);
    0
}
