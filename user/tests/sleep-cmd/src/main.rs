//! sleep - suspend execution for N ticks.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: sleep ticks");
        return 1;
    }
    let arg = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
    let mut n: i32 = 0;
    for &b in arg.as_bytes() {
        if b < b'0' || b > b'9' { break; }
        n = n * 10 + (b - b'0') as i32;
    }
    syscall::sleep(n);
    0
}
