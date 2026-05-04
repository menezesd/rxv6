#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(child-inherit) begin");
    // Open some files in parent
    syscall::create(b"ci1.dat\0", 0);
    let fd1 = syscall::open(b"ci1.dat\0");
    let fd2 = syscall::open(b"ci1.dat\0");
    // Exec child - it should have clean FD table
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    let s = syscall::wait(pid);
    // Parent's FDs should still work
    syscall::write(fd1, b"ok");
    syscall::close(fd1);
    syscall::close(fd2);
    if s == 81 {
        println!("(child-inherit) PASSED");
    }
    0
}
