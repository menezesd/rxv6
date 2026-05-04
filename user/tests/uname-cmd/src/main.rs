//! uname - print system information.
//!
//! Flags: -s (sysname), -n (nodename), -r (release), -m (machine), -a (all)

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::print;
use rxv6_user::println;

const SYSNAME: &str = "rxv6";
const NODENAME: &str = "rxv6";
const RELEASE: &str = "0.1.0";
const MACHINE: &str = "i686";

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc <= 1 {
        println!("{}", SYSNAME);
        return 0;
    }

    let mut show_s = false;
    let mut show_n = false;
    let mut show_r = false;
    let mut show_m = false;

    for i in 1..argc {
        let arg = unsafe { rxv6_user::cstr_to_str(*argv.add(i as usize)) };
        for b in arg.bytes() {
            match b {
                b'a' => { show_s = true; show_n = true; show_r = true; show_m = true; }
                b's' => show_s = true,
                b'n' => show_n = true,
                b'r' => show_r = true,
                b'm' => show_m = true,
                b'-' => {}
                _ => {}
            }
        }
    }

    if !show_s && !show_n && !show_r && !show_m { show_s = true; }

    let mut first = true;
    if show_s { if !first { print!(" "); } print!("{}", SYSNAME); first = false; }
    if show_n { if !first { print!(" "); } print!("{}", NODENAME); first = false; }
    if show_r { if !first { print!(" "); } print!("{}", RELEASE); first = false; }
    if show_m { if !first { print!(" "); } print!("{}", MACHINE); }
    println!("");
    0
}
