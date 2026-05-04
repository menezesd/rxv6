#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"mf.dat\0", 0);
    let fd = syscall::open(b"mf.dat\0");
    syscall::write(fd, b"PARENT");
    let p1 = syscall::exec(b"child-simple\0" as *const u8);
    let p2 = syscall::exec(b"child-simple\0" as *const u8);
    syscall::wait(p1);
    syscall::wait(p2);
    syscall::seek(fd, 0);
    let mut buf = [0u8; 6];
    let n = syscall::read(fd, &mut buf);
    if n == 6 && &buf == b"PARENT" {
        println!("(multi-child-fd) PASSED");
    }
    syscall::close(fd);
    0
}
