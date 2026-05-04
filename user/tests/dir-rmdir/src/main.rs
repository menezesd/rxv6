#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::mkdir(b"/rmtest\0");
    let ok = syscall::remove(b"/rmtest\0");
    let fd = syscall::open(b"/rmtest\0");
    if ok && fd < 0 {
        println!("(dir-rmdir) PASSED");
    }
    0
}
