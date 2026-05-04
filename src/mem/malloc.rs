#![allow(dead_code)]
//! Kernel heap allocator.
//!
//! Provides a `#[global_allocator]` backed by `palloc`.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use super::palloc::{self, PallocFlags};
use super::vaddr::PGSIZE;
use crate::sync::InterruptGuard;

/// Initial heap size in pages.
const HEAP_INIT_PAGES: usize = 256; // 1 MB

/// Simple block allocator.
///
/// Large allocations (>= PGSIZE) go directly to palloc.
/// Small allocations come from a free-list managed region.
struct KernelAllocator;

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;

/// A free block in the heap free list.
#[repr(C)]
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

/// Heap state — protected by interrupt disable.
static mut HEAP_START: *mut u8 = ptr::null_mut();
static mut HEAP_SIZE: usize = 0;
static mut FREE_LIST: *mut FreeBlock = ptr::null_mut();
static mut HEAP_INITIALISED: bool = false;

/// Initialise the kernel heap. Must be called after `palloc::init`.
pub fn init() {
    unsafe {
        let pages = palloc::get_multiple(
            PallocFlags::ASSERT | PallocFlags::ZERO,
            HEAP_INIT_PAGES,
        );
        assert!(!pages.is_null(), "malloc::init: failed to allocate heap");

        HEAP_START = pages;
        HEAP_SIZE = HEAP_INIT_PAGES * PGSIZE;

        // Initialise the free list with one big block.
        let block = pages as *mut FreeBlock;
        (*block).size = HEAP_SIZE;
        (*block).next = ptr::null_mut();
        FREE_LIST = block;
        HEAP_INITIALISED = true;
    }
}

/// Align `val` up to `align` (must be a power of two).
fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

/// Safety: all allocations are protected by InterruptGuard (disabling interrupts),
/// which provides mutual exclusion in this single-core kernel. Large allocations
/// (>= PGSIZE) delegate to palloc; small ones use a first-fit free-list.
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !HEAP_INITIALISED {
            // Before the heap is set up, we can only satisfy page-aligned
            // allocations directly from palloc.
            let pages = layout.size().div_ceil(PGSIZE);
            return palloc::get_multiple(PallocFlags::ZERO, pages);
        }

        let _guard = InterruptGuard::new();

        let align = layout.align().max(core::mem::align_of::<FreeBlock>());
        // Minimum allocation: enough room for a FreeBlock so we can free it.
        let size = align_up(
            layout.size().max(core::mem::size_of::<FreeBlock>()),
            align,
        );

        // Large allocation: go straight to palloc.
        if size >= PGSIZE {
            let page_cnt = size.div_ceil(PGSIZE);
            return palloc::get_multiple(PallocFlags::ZERO, page_cnt);
        }

        // First-fit search on the free list.
        let mut prev: *mut FreeBlock = ptr::null_mut();
        let mut cur = FREE_LIST;

        while !cur.is_null() {
            // Compute aligned start within this block.
            let block_start = cur as usize;
            let data_start = align_up(block_start, align);
            let padding = data_start - block_start;
            let needed = size + padding;

            if (*cur).size >= needed {
                let remaining = (*cur).size - needed;
                if remaining >= core::mem::size_of::<FreeBlock>() {
                    // Split: put the leftover at the end of the block.
                    let new_block = (block_start + needed) as *mut FreeBlock;
                    (*new_block).size = remaining;
                    (*new_block).next = (*cur).next;
                    if prev.is_null() {
                        FREE_LIST = new_block;
                    } else {
                        (*prev).next = new_block;
                    }
                } else {
                    // Use the whole block.
                    if prev.is_null() {
                        FREE_LIST = (*cur).next;
                    } else {
                        (*prev).next = (*cur).next;
                    }
                }
                return data_start as *mut u8;
            }

            prev = cur;
            cur = (*cur).next;
        }

        // No suitable block found.
        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let align = layout.align().max(core::mem::align_of::<FreeBlock>());
        let size = align_up(
            layout.size().max(core::mem::size_of::<FreeBlock>()),
            align,
        );

        // Large allocation: return to palloc.
        if size >= PGSIZE {
            let page_cnt = size.div_ceil(PGSIZE);
            palloc::free_multiple(ptr, page_cnt);
            return;
        }

        let _guard = InterruptGuard::new();

        // Prepend to free list.
        let block = ptr as *mut FreeBlock;
        (*block).size = size;
        (*block).next = FREE_LIST;
        FREE_LIST = block;
    }
}

/// Required by `alloc` crate when allocation fails.
#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!(
        "kernel heap allocation failed: size={}, align={}",
        layout.size(),
        layout.align()
    );
}
