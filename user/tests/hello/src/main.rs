#![no_std]
#![no_main]

use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(hello) begin");
    println!("Hello from Rust user program!");
    println!("(hello) end");
    println!("(hello) PASSED");
    0
}
