#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-bad-read) begin");
    let bad = 0xBFFF0000u32 as *const u8; // high user address, unmapped
    let _ = unsafe { core::ptr::read_volatile(bad) };
    println!("(pt-bad-read) FAILED");
    0
}
