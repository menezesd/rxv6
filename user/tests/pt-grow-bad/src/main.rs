#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-grow-bad) begin");
    // Access far below the stack - should NOT grow
    let bad_ptr = 0x1000 as *const u8;
    let _ = unsafe { core::ptr::read_volatile(bad_ptr) };
    println!("(pt-grow-bad) FAILED - should have been killed");
    0
}
