#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(child-rox) begin");
    // Start a child that takes a while
    let pid = syscall::exec(b"qsort\0" as *const u8);
    // Try to write to qsort's executable while it runs
    let fd = syscall::open(b"qsort\0");
    if fd >= 0 {
        let _n = syscall::write(fd, b"HACK");
        syscall::close(fd);
        // n should be 0 (denied) while child is running
    }
    let s = syscall::wait(pid);
    if s == 0 {
        println!("(child-rox) PASSED");
    }
    0
}
