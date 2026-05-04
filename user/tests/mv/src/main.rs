//! mv - move (rename) files.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 3 {
        println!("usage: mv old new");
        return 1;
    }
    let old = unsafe { *argv.add(1) };
    let new = unsafe { *argv.add(2) };

    if syscall::link(old, new) < 0 {
        println!("mv: cannot link");
        return 1;
    }
    if syscall::unlink(old) < 0 {
        println!("mv: cannot unlink old");
        return 1;
    }
    0
}
