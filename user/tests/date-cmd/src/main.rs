//! date - print system uptime.
#![no_std]
#![no_main]
use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let ticks = syscall::uptime();
    let secs = ticks / 100;
    let mins = secs / 60;
    let hours = mins / 60;
    println!("uptime: {}h {}m {}s ({} ticks)", hours, mins % 60, secs % 60, ticks);
    0
}
