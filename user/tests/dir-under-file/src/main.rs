#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-under-file) begin");
    syscall::create(b"notdir\0", 0);
    let ok = syscall::create(b"/notdir/sub\0", 0);
    if !ok {
        println!("(dir-under-file) PASSED");
    } else {
        println!("(dir-under-file) FAILED");
    }
    0
}
