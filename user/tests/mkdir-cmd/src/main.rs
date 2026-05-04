//! mkdir - create directories.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: mkdir dir ...");
        return 1;
    }
    for i in 1..argc {
        let path = unsafe { *argv.add(i as usize) };
        if syscall::mkdir(path) < 0 {
            let name = unsafe { rxv6_user::cstr_to_str(path) };
            println!("mkdir: {} failed", name);
            return 1;
        }
    }
    0
}
