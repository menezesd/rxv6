#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-grow-stack) begin");
    // Allocate array larger than one page on the stack to force stack growth
    let mut big: [u8; 4096] = [0; 4096];
    for i in 0..4096 {
        big[i] = (i & 0xFF) as u8;
    }
    // Verify
    let mut ok = true;
    for i in 0..4096 {
        if big[i] != (i & 0xFF) as u8 {
            ok = false;
            break;
        }
    }
    if ok {
        println!("(pt-grow-stack) PASSED");
    } else {
        println!("(pt-grow-stack) FAILED");
    }
    0
}
