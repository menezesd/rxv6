#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(page-merge-stk) begin");
    let mut big: [u8; 8192] = [0; 8192]; // forces multiple stack pages
    for i in 0..8192 {
        big[i] = (i & 0xFF) as u8;
    }
    let mut ok = true;
    for i in 0..8192 {
        if big[i] != (i & 0xFF) as u8 {
            ok = false;
            break;
        }
    }
    if ok {
        println!("(page-merge-stk) PASSED");
    }
    0
}
