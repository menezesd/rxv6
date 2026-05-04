#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let ok = syscall::create(b"sm.dat\0", 512);
    if ok {
        println!("(sm-create) PASSED");
    }
    0
}
