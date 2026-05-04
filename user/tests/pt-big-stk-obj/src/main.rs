#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    // Allocate 64KB on the stack
    let big: [u8; 65536] = [0x42; 65536];
    // Verify it wasn't corrupted
    if big[0] == 0x42 && big[65535] == 0x42 {
        println!("(pt-big-stk-obj) PASSED");
    }
    0
}
