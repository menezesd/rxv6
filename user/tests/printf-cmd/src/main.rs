//! printf - format and print data.
//!
//! Usage: printf format [arg ...]
//! Supports: %s %d %c %% \n \t \\ \xNN

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::print;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        syscall::write(2, b"usage: printf format [arg ...]\n");
        return 1;
    }

    let fmt = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
    let mut arg_idx = 2i32;

    let bytes = fmt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'n' => { syscall::write(1, b"\n"); }
                b't' => { syscall::write(1, b"\t"); }
                b'\\' => { syscall::write(1, b"\\"); }
                b'0' => { syscall::write(1, &[0]); }
                b'x' if i + 2 < bytes.len() => {
                    let hi = hex_digit(bytes[i + 1]);
                    let lo = hex_digit(bytes[i + 2]);
                    syscall::write(1, &[(hi << 4) | lo]);
                    i += 2;
                }
                c => { syscall::write(1, &[b'\\', c]); }
            }
            i += 1;
        } else if bytes[i] == b'%' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b's' => {
                    if arg_idx < argc {
                        let s = unsafe { rxv6_user::cstr_to_str(*argv.add(arg_idx as usize)) };
                        syscall::write(1, s.as_bytes());
                        arg_idx += 1;
                    }
                }
                b'd' => {
                    if arg_idx < argc {
                        let s = unsafe { rxv6_user::cstr_to_str(*argv.add(arg_idx as usize)) };
                        let n = atoi(s);
                        print!("{}", n);
                        arg_idx += 1;
                    }
                }
                b'c' => {
                    if arg_idx < argc {
                        let s = unsafe { rxv6_user::cstr_to_str(*argv.add(arg_idx as usize)) };
                        if !s.is_empty() {
                            syscall::write(1, &[s.as_bytes()[0]]);
                        }
                        arg_idx += 1;
                    }
                }
                b'%' => { syscall::write(1, b"%"); }
                c => { syscall::write(1, &[b'%', c]); }
            }
            i += 1;
        } else {
            syscall::write(1, &[bytes[i]]);
            i += 1;
        }
    }
    0
}

fn hex_digit(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

fn atoi(s: &str) -> i32 {
    let b = s.as_bytes();
    let mut n: i32 = 0;
    let mut i = 0;
    let neg = !b.is_empty() && b[0] == b'-';
    if neg { i = 1; }
    while i < b.len() && b[i] >= b'0' && b[i] <= b'9' {
        n = n * 10 + (b[i] - b'0') as i32;
        i += 1;
    }
    if neg { -n } else { n }
}
