#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(grow-seq-lg) begin");
    syscall::create(b"gsl.dat\0", 0);
    let fd = syscall::open(b"gsl.dat\0");
    let block = [0xEEu8; 512];
    for _ in 0..20 {
        syscall::write(fd, &block);
    } // 10KB
    let sz = syscall::filesize(fd);
    syscall::close(fd);
    if sz == 10240 {
        println!("(grow-seq-lg) PASSED");
    } else {
        println!("(grow-seq-lg) FAILED sz={}", sz);
    }
    0
}
