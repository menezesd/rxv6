#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-vine) begin");
    syscall::mkdir(b"/v1\0");
    syscall::mkdir(b"/v1/v2\0");
    syscall::mkdir(b"/v1/v2/v3\0");
    syscall::mkdir(b"/v1/v2/v3/v4\0");
    syscall::create(b"/v1/v2/v3/v4/end\0", 0);
    let fd = syscall::open(b"/v1/v2/v3/v4/end\0");
    if fd >= 0 {
        println!("(dir-vine) PASSED");
        syscall::close(fd);
    } else {
        println!("(dir-vine) FAILED");
    }
    0
}
