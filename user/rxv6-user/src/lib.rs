//! rxv6 user-space runtime library.
//!
//! Provides syscall wrappers, entry point, heap allocator, and basic I/O
//! for user programs.

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod syscall;

// ---- Heap allocator using sbrk() ----

use core::alloc::{GlobalAlloc, Layout};

struct SbrkAllocator;

unsafe impl GlobalAlloc for SbrkAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        // sbrk to get memory; over-allocate to handle alignment
        let total = size + align + 4; // 4 bytes for storing allocated size
        let base = syscall::sbrk(total as i32);
        if base < 0 { return core::ptr::null_mut(); }
        let base = base as usize;
        // Align the returned pointer (leave room for size header)
        let aligned = (base + 4 + align - 1) & !(align - 1);
        // Store the allocation size just before the aligned pointer
        *((aligned - 4) as *mut u32) = total as u32;
        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Simple bump allocator: dealloc is a no-op.
        // Memory is reclaimed when the process exits.
    }
}

#[global_allocator]
static ALLOCATOR: SbrkAllocator = SbrkAllocator;

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    syscall::write(2, b"alloc error\n");
    syscall::exit(1);
}

use core::fmt;

// Entry point: the kernel places [fake_ret_addr] [argc] [argv] on the stack.
core::arch::global_asm!(
    ".global _start",
    "_start:",
    "pop eax",            // discard fake return address
    "call rust_main",     // rust_main(argc, argv) via cdecl
    "push eax",           // push exit code (arg0)
    "push 2",             // SYS_EXIT
    "int 0x80",           // exit syscall
    "jmp _start",         // unreachable
);

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

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("panic: {}", info);
    syscall::exit(1);
}
