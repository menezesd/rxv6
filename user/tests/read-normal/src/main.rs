#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(read-normal) begin");
    let data = b"Read me!";
    syscall::create(b"readfile.txt\0", 0);
    let wfd = syscall::open(b"readfile.txt\0");
    syscall::write(wfd, data);
    syscall::close(wfd);

    let fd = syscall::open(b"readfile.txt\0");
    let mut buf = [0u8; 16];
    let n = syscall::read(fd, &mut buf[..data.len()]);
    if n == data.len() as i32 && &buf[..data.len()] == data {
        println!("(read-normal) PASSED");
    } else {
        println!("(read-normal) FAILED: read {} bytes", n);
    }
    syscall::close(fd);
    println!("(read-normal) end");
    0
}
