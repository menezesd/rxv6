#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(bad-jump2) begin");
    let f: fn() = unsafe { core::mem::transmute(0u32) };
    f();
    println!("(bad-jump2) FAILED - should not reach here");
    0
}
