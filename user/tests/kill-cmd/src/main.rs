//! kill - send signal to a process.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: kill pid ...");
        return 1;
    }
    for i in 1..argc {
        let arg = unsafe { rxv6_user::cstr_to_str(*argv.add(i as usize)) };
        let pid = atoi(arg);
        if syscall::kill(pid) < 0 {
            println!("kill: failed for pid {}", pid);
        }
    }
    0
}

fn atoi(s: &str) -> i32 {
    let mut n: i32 = 0;
    let mut neg = false;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        if i == 0 && b == b'-' { neg = true; continue; }
        if b < b'0' || b > b'9' { break; }
        n = n * 10 + (b - b'0') as i32;
    }
    if neg { -n } else { n }
}
