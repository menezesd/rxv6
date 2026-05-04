#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(read-write) begin");
    syscall::create(b"rw.txt\0", 0);
    let fd = syscall::open(b"rw.txt\0");
    syscall::write(fd, b"Hello filesystem!");
    syscall::seek(fd, 0);
    let mut buf = [0u8; 32];
    let n = syscall::read(fd, &mut buf[..17]);
    let s = core::str::from_utf8(&buf[..n as usize]).unwrap_or("???");
    if s == "Hello filesystem!" { println!("(read-write) PASSED"); } else { println!("(read-write) FAILED: {}", s); }
    syscall::close(fd);
    0
}
