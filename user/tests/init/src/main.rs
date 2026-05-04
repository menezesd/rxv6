//! init - the first user process (PID 1).
//!
//! Opens the console device for stdin/stdout/stderr,
//! then forks and execs the shell in an infinite loop.
//! If the shell exits, init forks a new one.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("init: starting sh");

    loop {
        let pid = syscall::fork();
        if pid < 0 {
            println!("init: fork failed");
            syscall::exit(1);
        }
        if pid == 0 {
            // Child: exec the shell
            let path = b"sh\0";
            let argv: [*const u8; 2] = [path.as_ptr(), core::ptr::null()];
            syscall::exec(path.as_ptr(), argv.as_ptr());
            println!("init: exec sh failed");
            syscall::exit(1);
        }
        // Parent: wait for shell to exit, then restart
        let mut status: i32 = 0;
        loop {
            let wpid = syscall::wait(&mut status as *mut i32);
            if wpid == pid || wpid < 0 {
                break;
            }
        }
    }
}
