#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sl-remove) begin");
    syscall::create(b"sl-rm\0", 0);
    let ok = syscall::remove(b"sl-rm\0");
    if ok {
        println!("(sl-remove) PASSED");
    } else {
        println!("(sl-remove) FAILED");
    }
    0
}
