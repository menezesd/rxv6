//! kill - send signal to a process.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: kill [-signal] pid ...");
        return 1;
    }
    let mut sig = syscall::SIGTERM;
    let mut start = 1;
    // Check for -signal flag
    let first = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
    if first.as_bytes().first() == Some(&b'-') && first.len() > 1 {
        sig = atoi(&first[1..]) as u32;
        start = 2;
    }
    for i in start..argc {
        let arg = unsafe { rxv6_user::cstr_to_str(*argv.add(i as usize)) };
        let pid = atoi(arg);
        if syscall::kill(pid, sig) < 0 {
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
