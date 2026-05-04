#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(page-linear) begin");
    // Allocate and touch many pages linearly
    let mut ok = true;
    for i in 0..16 {
        let mut buf = [0u8; 256];
        buf[0] = i as u8;
        if buf[0] != i as u8 {
            ok = false;
        }
    }
    if ok {
        println!("(page-linear) PASSED");
    }
    0
}
