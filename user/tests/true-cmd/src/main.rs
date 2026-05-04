#![no_std]
#![no_main]
extern crate rxv6_user;
#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 { 0 }
