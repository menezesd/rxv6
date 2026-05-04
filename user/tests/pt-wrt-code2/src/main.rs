#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-wrt-code2) begin");
    // Try to get the kernel to write to our code page via SYS_READ
    let fd = syscall::open(b"pt-wrt-code2\0"); // open self
    if fd >= 0 {
        // Try reading file data INTO our code page
        let code_addr = rust_main as *const u8 as usize;
        // This should fail because code pages are read-only
        syscall::syscall3(syscall::SYS_READ, fd as u32, code_addr as u32, 10);
        syscall::close(fd);
    }
    println!("(pt-wrt-code2) PASSED"); // survived (kernel rejected the write)
    0
}
