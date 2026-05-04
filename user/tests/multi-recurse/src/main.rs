#![no_std]
#![no_main]
extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let depth = if argc > 1 {
        let arg = unsafe { rustos_user::cstr_to_str(*argv.add(1)) };
        arg.parse::<i32>().unwrap_or(0)
    } else {
        0
    };
    if depth < 3 {
        let pid = syscall::exec(b"child-simple\0" as *const u8);
        let s = syscall::wait(pid);
        if s == 81 {
            println!("(multi-recurse) depth {} ok", depth);
        }
    }
    println!("(multi-recurse) PASSED");
    0
}
