#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(rox-child) begin");
    // Verify we can't write to our OWN executable (same as rox-simple),
    // and also exec a child to verify child runs correctly.
    let fd = syscall::open(b"rox-child\0");
    let self_protected = if fd >= 0 {
        let n = syscall::write(fd, b"HACK");
        syscall::close(fd);
        n == 0
    } else { true };

    let pid = syscall::exec(b"child-simple\0" as *const u8);
    let child_status = syscall::wait(pid);

    if self_protected && child_status == 81 {
        println!("(rox-child) PASSED");
    } else {
        println!("(rox-child) FAILED: self_protected={}, child_status={}", self_protected, child_status);
    }
    0
}
