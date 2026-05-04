#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-rm-parent) begin");
    syscall::mkdir(b"/parent\0");
    syscall::mkdir(b"/parent/child\0");
    // Remove parent with child (should fail - non-empty)
    let ok = syscall::remove(b"/parent\0");
    if !ok {
        println!("(dir-rm-parent) PASSED");
    } else {
        println!("(dir-rm-parent) FAILED");
    }
    0
}
