#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(bad-write2) begin");
    unsafe { core::ptr::write_volatile(0 as *mut u8, 0); }
    println!("(bad-write2) FAILED - should not reach here");
    0
}
