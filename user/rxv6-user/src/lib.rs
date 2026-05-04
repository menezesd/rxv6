//! rxv6 user-space runtime library.
//!
//! Provides syscall wrappers, entry point, heap allocator, and basic I/O
//! for user programs.

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod syscall;

// ---- Free-list heap allocator using sbrk() ----
//
// Classic K&R-style allocator: each block has a header with size.
// Free blocks are chained in a singly-linked list, sorted by address.
// alloc() searches free list (first-fit), splits if oversized.
// dealloc() returns block to free list, coalescing neighbors.

use core::alloc::{GlobalAlloc, Layout};

/// Block header: sits just before the returned pointer.
/// `size` is the total block size including the header (in bytes).
/// `next` points to the next free block (null if end of list or allocated).
#[repr(C)]
struct BlockHeader {
    size: usize,
    next: *mut BlockHeader,
}

const HEADER_SIZE: usize = core::mem::size_of::<BlockHeader>();
// Minimum allocation unit: header + 8 bytes (ensures alignment)
const MIN_BLOCK: usize = HEADER_SIZE + 8;

struct FreeListAllocator {
    head: core::cell::UnsafeCell<*mut BlockHeader>,
}

unsafe impl Sync for FreeListAllocator {}

unsafe impl GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(HEADER_SIZE); // at least header-aligned
        // Total size: header + padding for alignment + payload
        let payload = layout.size();
        let total = (HEADER_SIZE + align - 1 + payload).max(MIN_BLOCK);
        // Round up to HEADER_SIZE alignment
        let total = (total + HEADER_SIZE - 1) & !(HEADER_SIZE - 1);

        let head = &mut *self.head.get();

        // First-fit search through free list
        let mut prev: *mut BlockHeader = core::ptr::null_mut();
        let mut cur = *head;
        while !cur.is_null() {
            if (*cur).size >= total {
                // Found a fit. Split if remainder is large enough.
                let remainder = (*cur).size - total;
                if remainder >= MIN_BLOCK {
                    // Split: shrink current block, carve new block from the end
                    (*cur).size = remainder;
                    let new_block = (cur as *mut u8).add(remainder) as *mut BlockHeader;
                    (*new_block).size = total;
                    (*new_block).next = core::ptr::null_mut();
                    return (new_block as *mut u8).add(HEADER_SIZE);
                } else {
                    // Use entire block
                    if prev.is_null() {
                        *head = (*cur).next;
                    } else {
                        (*prev).next = (*cur).next;
                    }
                    (*cur).next = core::ptr::null_mut();
                    return (cur as *mut u8).add(HEADER_SIZE);
                }
            }
            prev = cur;
            cur = (*cur).next;
        }

        // No free block found — grow heap with sbrk
        let grow = total.max(4096); // grow at least one page
        let base = syscall::sbrk(grow as i32);
        if base < 0 {
            return core::ptr::null_mut();
        }
        let block = base as usize as *mut BlockHeader;
        (*block).size = grow;
        (*block).next = core::ptr::null_mut();

        if grow > total && grow - total >= MIN_BLOCK {
            // Put remainder on free list
            let remainder_block = (block as *mut u8).add(total) as *mut BlockHeader;
            (*remainder_block).size = grow - total;
            (*remainder_block).next = core::ptr::null_mut();
            free_list_insert(head, remainder_block);

            (*block).size = total;
        }

        (block as *mut u8).add(HEADER_SIZE)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() { return; }
        let block = ptr.sub(HEADER_SIZE) as *mut BlockHeader;
        let head = &mut *self.head.get();
        free_list_insert(head, block);
    }
}

/// Insert a block into the free list in address order, coalescing neighbors.
unsafe fn free_list_insert(head: &mut *mut BlockHeader, block: *mut BlockHeader) {
    let block_addr = block as usize;
    let block_end = block_addr + (*block).size;

    // Find insertion point (sorted by address)
    let mut prev: *mut BlockHeader = core::ptr::null_mut();
    let mut cur = *head;
    while !cur.is_null() && (cur as usize) < block_addr {
        prev = cur;
        cur = (*cur).next;
    }

    // Try to coalesce with next block
    if !cur.is_null() && block_end == cur as usize {
        (*block).size += (*cur).size;
        (*block).next = (*cur).next;
    } else {
        (*block).next = cur;
    }

    // Try to coalesce with previous block
    if !prev.is_null() {
        let prev_end = prev as usize + (*prev).size;
        if prev_end == block_addr {
            (*prev).size += (*block).size;
            (*prev).next = (*block).next;
        } else {
            (*prev).next = block;
        }
    } else {
        *head = block;
    }
}

#[global_allocator]
static ALLOCATOR: FreeListAllocator = FreeListAllocator {
    head: core::cell::UnsafeCell::new(core::ptr::null_mut()),
};

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
