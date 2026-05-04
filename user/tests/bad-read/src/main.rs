#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(bad-read) begin");
    let ptr = 0xC0000000u32 as *const u8;
    let _ = unsafe { core::ptr::read_volatile(ptr) };
    println!("(bad-read) FAILED - should not reach here");
    0
}
