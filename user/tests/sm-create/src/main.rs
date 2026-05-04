#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let ok = syscall::create(b"sm.dat\0", 512);
    if ok {
        println!("(sm-create) PASSED");
    }
    0
}
