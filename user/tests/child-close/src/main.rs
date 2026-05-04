#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"test.dat\0", 100);
    let fd = syscall::open(b"test.dat\0");
    syscall::write(fd, b"hello");
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    syscall::wait(pid);
    // Verify parent's FD still works
    syscall::seek(fd, 0);
    let mut buf = [0u8; 5];
    let n = syscall::read(fd, &mut buf);
    if n == 5 {
        println!("(child-close) PASSED");
    } else {
        println!("(child-close) FAILED");
    }
    syscall::close(fd);
    0
}
