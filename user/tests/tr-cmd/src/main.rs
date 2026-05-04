//! tr - translate characters.
//!
//! Usage: tr set1 set2
//! Reads stdin, replaces each character in set1 with corresponding character in set2.
//! Supports a-z ranges.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        println!("usage: tr set1 set2");
        return 1;
    }
    let set1 = expand_set(unsafe { rxv6_user::cstr_to_str(*argv.add(1)) });
    let set2 = expand_set(unsafe { rxv6_user::cstr_to_str(*argv.add(2)) });

    // Build translation table
    let mut table = [0u8; 256];
    for i in 0..256 { table[i] = i as u8; }
    for i in 0..set1.len() {
        let replacement = if i < set2.len() { set2[i] } else if !set2.is_empty() { set2[set2.len() - 1] } else { set1[i] };
        table[set1[i] as usize] = replacement;
    }

    let mut buf = [0u8; 512];
    loop {
        let n = syscall::read(0, &mut buf);
        if n <= 0 { break; }
        for i in 0..n as usize {
            buf[i] = table[buf[i] as usize];
        }
        syscall::write(1, &buf[..n as usize]);
    }
    0
}

fn expand_set(s: &str) -> [u8; 256] {
    let bytes = s.as_bytes();
    let mut result = [0u8; 256];
    let mut len = 0;
    let mut i = 0;
    while i < bytes.len() && len < 256 {
        if i + 2 < bytes.len() && bytes[i + 1] == b'-' {
            let start = bytes[i];
            let end = bytes[i + 2];
            let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
            let mut c = lo;
            while c <= hi && len < 256 {
                result[len] = c;
                len += 1;
                if c == 255 { break; }
                c += 1;
            }
            i += 3;
        } else {
            result[len] = bytes[i];
            len += 1;
            i += 1;
        }
    }
    // Truncate unused portion to zeros (but len is tracked implicitly by content)
    result
}
