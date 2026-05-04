#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(remove) begin");
    syscall::create(b"rm.txt\0", 0);
    let ok = syscall::remove(b"rm.txt\0");
    let fd = syscall::open(b"rm.txt\0");
    if ok && fd < 0 { println!("(remove) PASSED"); } else { println!("(remove) FAILED ok={} fd={}", ok, fd); }
    println!("(remove) end");
    0
}
