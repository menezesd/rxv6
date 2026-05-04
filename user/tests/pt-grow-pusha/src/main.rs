#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(pt-grow-pusha) begin");
    // Grow the stack by one page using inline asm (like a real alloca).
    // We decrement ESP first, then access the new stack area.
    // The page fault occurs with ESP already pointing into the new page,
    // so the stack growth check (fault >= esp - 32) passes.
    unsafe {
        let val: u32;
        core::arch::asm!(
            "sub esp, 4096",        // grow stack into next page
            "mov dword ptr [esp], 0x12345678", // write to new stack page (may fault)
            "mov {out}, [esp]",     // read back
            "add esp, 4096",        // restore stack
            out = out(reg) val,
            options(nostack),
        );
        if val == 0x12345678 {
            println!("(pt-grow-pusha) PASSED");
        } else {
            println!("(pt-grow-pusha) FAILED");
        }
    }
    0
}
