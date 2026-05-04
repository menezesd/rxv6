//! seq - print a sequence of numbers.
//!
//! Usage: seq [first [step]] last

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let (first, step, last) = match argc {
        2 => {
            let last = atoi(arg(argv, 1));
            (1, 1, last)
        }
        3 => {
            let first = atoi(arg(argv, 1));
            let last = atoi(arg(argv, 2));
            (first, if first <= last { 1 } else { -1 }, last)
        }
        4 => {
            (atoi(arg(argv, 1)), atoi(arg(argv, 2)), atoi(arg(argv, 3)))
        }
        _ => {
            println!("usage: seq [first [step]] last");
            return 1;
        }
    };

    if step == 0 {
        println!("seq: zero step");
        return 1;
    }

    let mut i = first;
    if step > 0 {
        while i <= last {
            println!("{}", i);
            i += step;
        }
    } else {
        while i >= last {
            println!("{}", i);
            i += step;
        }
    }
    0
}

fn arg(argv: *const *const u8, i: usize) -> &'static str {
    unsafe { rxv6_user::cstr_to_str(*argv.add(i)) }
}

fn atoi(s: &str) -> i32 {
    let bytes = s.as_bytes();
    let mut n: i32 = 0;
    let mut i = 0;
    let neg = !bytes.is_empty() && bytes[0] == b'-';
    if neg { i = 1; }
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        n = n * 10 + (bytes[i] - b'0') as i32;
        i += 1;
    }
    if neg { -n } else { n }
}
