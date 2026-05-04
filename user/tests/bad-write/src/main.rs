#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(bad-write) begin");
    let ptr = 0xC0000000u32 as *mut u8;
    unsafe { core::ptr::write_volatile(ptr, 0); }
    println!("(bad-write) FAILED - should not reach here");
    0
}
