#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(lg-create) begin");
    let ok = syscall::create(b"bigfile\0", 4096);
    if ok {
        println!("(lg-create) PASSED");
    } else {
        println!("(lg-create) FAILED");
    }
    0
}
