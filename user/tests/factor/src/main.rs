//! factor - print prime factors of a number.
//!
//! Usage: factor 12      -> 12: 2 2 3
//!        factor 97      -> 97: 97
//!        factor          (interactive: reads numbers from stdin)

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc > 1 {
        for i in 1..argc as usize {
            let s = unsafe { rxv6_user::cstr_to_str(*argv.add(i)) };
            let n = atoi(s);
            if n < 1 {
                println!("{}: not a positive integer", s);
            } else {
                factorize(n as u64);
            }
        }
    } else {
        // Interactive mode
        let mut buf = [0u8; 64];
        loop {
            let n = getline(&mut buf);
            if n <= 0 { break; }
            let len = buf.iter().position(|&b| b == 0 || b == b'\n').unwrap_or(n as usize);
            if len == 0 { continue; }
            let s = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
            let num = atoi(s);
            if num < 1 { println!("{}: not valid", s); continue; }
            factorize(num as u64);
        }
    }
    0
}

fn factorize(mut n: u64) {
    print!("{}:", n);
    if n <= 1 {
        println!(" {}", n);
        return;
    }

    // Trial division
    let mut d: u64 = 2;
    while d * d <= n {
        while n % d == 0 {
            print!(" {}", d);
            n /= d;
        }
        d += if d == 2 { 1 } else { 2 };
    }
    if n > 1 {
        print!(" {}", n);
    }
    println!();
}

fn atoi(s: &str) -> i64 {
    let mut n: i64 = 0;
    let mut neg = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' { i += 1; }
    if i < bytes.len() && bytes[i] == b'-' { neg = true; i += 1; }
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        n = n * 10 + (bytes[i] - b'0') as i64;
        i += 1;
    }
    if neg { -n } else { n }
}

fn getline(buf: &mut [u8]) -> i32 {
    let mut i = 0;
    while i < buf.len() - 1 {
        let mut c = [0u8; 1];
        let n = syscall::read(0, &mut c);
        if n <= 0 { return if i > 0 { i as i32 } else { -1 }; }
        if c[0] == b'\n' { break; }
        buf[i] = c[0];
        i += 1;
    }
    buf[i] = 0;
    i as i32
}
