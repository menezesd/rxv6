#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(sc-bad-arg) begin");
    // Place the syscall number at the very top of user address space
    // so arguments would extend into kernel space.
    // PHYS_BASE = 0xC0000000; put esp at PHYS_BASE - 4 so the syscall
    // number is readable but the first argument is not.
    unsafe {
        core::arch::asm!(
            "mov esp, 0xbffffffc",
            "mov dword ptr [esp], 1",  // SYS_EXIT
            "int 0x30",
            options(noreturn)
        );
    }
}
