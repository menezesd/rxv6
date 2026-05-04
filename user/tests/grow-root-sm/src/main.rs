#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(grow-root-sm) begin");
    let mut ok = true;
    // Create a few files with unique names to test root directory growth
    let names: [&[u8]; 3] = [b"grs1.dat\0", b"grs2.dat\0", b"grs3.dat\0"];
    for name in &names {
        if !syscall::create(*name, 0) {
            ok = false;
            break;
        }
    }
    if ok {
        println!("(grow-root-sm) PASSED");
    } else {
        println!("(grow-root-sm) FAILED");
    }
    0
}
