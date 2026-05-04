#![no_std]

pub mod syscall;

use core::fmt;

// Entry point: the kernel places [fake_ret_addr] [argc] [argv] on the stack.
// We pop the fake return address, then `call rust_main` pushes our own return
// address, leaving [ret_addr] [argc] [argv] -- exactly the cdecl layout for
// rust_main(argc, argv).
core::arch::global_asm!(
    ".global _start",
    "_start:",
    "pop eax",            // discard fake return address
    "call rust_main",     // rust_main(argc, argv) via cdecl
    "push eax",           // push exit code
    "push 1",             // SYS_EXIT
    "int 0x30",           // exit syscall
    "jmp _start",         // should never reach here
);

// User programs define a `#[no_mangle] pub extern "C" fn rust_main(argc, argv) -> i32`.
// We declare it here so the linker resolves it.
extern "C" {
    fn rust_main(argc: i32, argv: *const *const u8) -> i32;
}

// === Printing support ===

pub struct Stdout;

impl fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        syscall::write(1, s.as_bytes());
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = write!($crate::Stdout, $($arg)*);
        }
    };
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}

/// Read a C-string (null-terminated) from a raw pointer into a &str.
pub unsafe fn cstr_to_str(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
}

/// Test helper: print a test message in Pintos format.
pub fn msg(test_name: &str, message: &str) {
    print!("({}) {}\n", test_name, message);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("USER PANIC: {}", info);
    syscall::exit(-1);
}
