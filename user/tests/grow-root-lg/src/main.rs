#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(grow-root-lg) begin");
    let mut ok = true;
    let names: [&[u8]; 20] = [
        b"f00.txt\0", b"f01.txt\0", b"f02.txt\0", b"f03.txt\0", b"f04.txt\0",
        b"f05.txt\0", b"f06.txt\0", b"f07.txt\0", b"f08.txt\0", b"f09.txt\0",
        b"f10.txt\0", b"f11.txt\0", b"f12.txt\0", b"f13.txt\0", b"f14.txt\0",
        b"f15.txt\0", b"f16.txt\0", b"f17.txt\0", b"f18.txt\0", b"f19.txt\0",
    ];
    for name in &names {
        if !syscall::create(*name, 0) {
            ok = false;
            break;
        }
    }
    if ok {
        println!("(grow-root-lg) PASSED");
    } else {
        println!("(grow-root-lg) FAILED");
    }
    0
}
