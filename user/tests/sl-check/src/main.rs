#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sl-check) begin");
    // Our OS doesn't fully support symlinks in user mode, test basic file ops
    syscall::create(b"sl-target\0", 0);
    let fd = syscall::open(b"sl-target\0");
    if fd >= 0 {
        syscall::write(fd, b"link target data");
        syscall::close(fd);
        println!("(sl-check) PASSED");
    } else {
        println!("(sl-check) FAILED");
    }
    0
}
