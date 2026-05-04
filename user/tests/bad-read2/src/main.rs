#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(bad-read2) begin");
    unsafe { core::ptr::read_volatile(0 as *const u8); }
    println!("(bad-read2) FAILED - should not reach here");
    0
}
