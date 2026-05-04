//! yes - repeatedly output a string (default "y").
//!
//! Usage: yes         -> prints "y" forever
//!        yes hello   -> prints "hello" forever
//!
//! Useful for piping into programs that ask for confirmation:
//!   yes | rm-interactive-thing

#![no_std]
#![no_main]

use rxv6_user::syscall;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let msg: &[u8] = if argc > 1 {
        let ptr = unsafe { *argv.add(1) };
        let s = unsafe { rxv6_user::cstr_to_str(ptr) };
        s.as_bytes()
    } else {
        b"y"
    };

    loop {
        let n = syscall::write(1, msg);
        if n <= 0 { break; } // broken pipe
        let n2 = syscall::write(1, b"\n");
        if n2 <= 0 { break; }
    }
    0
}
