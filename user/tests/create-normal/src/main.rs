#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-normal) begin");
    let ok = syscall::create(b"cn-test.txt\0", 0);
    if ok { println!("(create-normal) PASSED"); } else { println!("(create-normal) FAILED"); }
    println!("(create-normal) end");
    0
}
