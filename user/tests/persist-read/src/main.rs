#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(persist-read) checking file...");
    let fd = syscall::open(b"persist.dat\0");
    if fd < 0 {
        println!("(persist-read) FAILED: file not found after reboot");
        return 1;
    }
    let mut buf = [0u8; 17];
    let n = syscall::read(fd, &mut buf);
    syscall::close(fd);
    if n == 17 && &buf == b"PERSISTED DATA OK" {
        println!("(persist-read) PASSED: data survived reboot!");
    } else {
        println!("(persist-read) FAILED: data corrupted (read {} bytes)", n);
    }
    0
}
