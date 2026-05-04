#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-bad-ptr) begin");
    // Pass a kernel-space pointer as filename - should kill the process
    let bad_ptr: *const u8 = 0xC0000000u32 as *const u8;
    let name_slice: &[u8] = unsafe { core::slice::from_raw_parts(bad_ptr, 1) };
    syscall::create(name_slice, 0);
    println!("(create-bad-ptr) FAILED: should have been killed");
    syscall::exit(-1);
}
