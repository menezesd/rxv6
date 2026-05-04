#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(dir-rm-tree) begin");
    syscall::mkdir(b"/tree\0");
    syscall::create(b"/tree/f.txt\0", 0);
    // Can't remove non-empty dir
    let ok = syscall::remove(b"/tree\0");
    if !ok {
        println!("(dir-rm-tree) PASSED: can't rm non-empty dir");
    } else {
        println!("(dir-rm-tree) FAILED");
    }
    0
}
