#![no_std]
#![no_main]

use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-bad-fd) begin");

    let mapid = syscall::mmap(99, 0x10000000);
    if mapid == -1 {
        println!("(mmap-bad-fd) PASSED");
    } else {
        println!("(mmap-bad-fd) FAILED: expected -1, got {}", mapid);
    }
    0
}
