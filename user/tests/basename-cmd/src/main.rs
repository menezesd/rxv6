//! basename - strip directory and suffix from filenames.
//!
//! Usage: basename path [suffix]

#![no_std]
#![no_main]

use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: basename path [suffix]");
        return 1;
    }
    let path = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
    let suffix = if argc >= 3 {
        unsafe { rxv6_user::cstr_to_str(*argv.add(2)) }
    } else { "" };

    // Strip trailing slashes
    let mut s = path.trim_end_matches('/');
    if s.is_empty() { println!("/"); return 0; }

    // Take last component
    if let Some(pos) = s.rfind('/') {
        s = &s[pos + 1..];
    }

    // Strip suffix
    if !suffix.is_empty() && s.len() > suffix.len() && s.ends_with(suffix) {
        s = &s[..s.len() - suffix.len()];
    }

    println!("{}", s);
    0
}
