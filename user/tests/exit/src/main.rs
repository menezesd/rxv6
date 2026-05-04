#![no_std]
#![no_main]

use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(exit) begin");
    syscall::exit(42);
}
