#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-mk-tree) begin");
    syscall::mkdir(b"/a\0");
    syscall::mkdir(b"/a/b\0");
    syscall::mkdir(b"/a/b/c\0");
    syscall::create(b"/a/b/c/f.txt\0", 0);
    let fd = syscall::open(b"/a/b/c/f.txt\0");
    if fd >= 0 {
        println!("(dir-mk-tree) PASSED");
        syscall::close(fd);
    } else {
        println!("(dir-mk-tree) FAILED");
    }
    0
}
