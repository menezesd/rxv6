#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-write-code) begin");
    let ptr = rust_main as *const u8 as *mut u8;
    unsafe {
        core::ptr::write_volatile(ptr, 0x90);
    }
    println!("(pt-write-code) FAILED - should have been killed");
    0
}
