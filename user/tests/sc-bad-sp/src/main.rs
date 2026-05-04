#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sc-bad-sp) begin");
    // Set ESP to kernel address and do a syscall - should be killed
    unsafe {
        core::arch::asm!(
            "mov esp, 0xC0000000",
            "push 0",     // SYS_HALT
            "int 0x30",
            options(noreturn)
        );
    }
}
