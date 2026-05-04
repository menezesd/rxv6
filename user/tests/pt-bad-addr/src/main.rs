#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-bad-addr) begin");
    let bad = 0x4000 as *const u8; // low but not zero, unmapped
    let _ = unsafe { core::ptr::read_volatile(bad) };
    println!("(pt-bad-addr) FAILED");
    0
}
