#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(child-syn-read) begin");
    syscall::create(b"synrd.dat\0", 0);
    let fd = syscall::open(b"synrd.dat\0");
    syscall::write(fd, b"shared data for sync read test!!");
    syscall::close(fd);
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    // Read while child is running
    let fd = syscall::open(b"synrd.dat\0");
    let mut buf = [0u8; 32];
    let n = syscall::read(fd, &mut buf);
    syscall::close(fd);
    let s = syscall::wait(pid);
    if n == 32 && s == 81 {
        println!("(child-syn-read) PASSED");
    } else {
        println!("(child-syn-read) FAILED n={} s={}", n, s);
    }
    0
}
