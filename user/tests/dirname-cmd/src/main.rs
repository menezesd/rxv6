//! dirname - strip last component from file name.
//!
//! Usage: dirname path

#![no_std]
#![no_main]

use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: dirname path");
        return 1;
    }
    let path = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };

    // Strip trailing slashes
    let s = path.trim_end_matches('/');
    if s.is_empty() { println!("/"); return 0; }

    match s.rfind('/') {
        Some(0) => println!("/"),
        Some(pos) => println!("{}", &s[..pos]),
        None => println!("."),
    }
    0
}
