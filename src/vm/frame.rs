use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::mem::palloc::{self, PallocFlags};
use crate::mem::vaddr::PGSIZE;
use crate::userprog::pagedir;
use crate::vm::swap;

/// A frame table entry tracks one physical frame.
pub struct FrameEntry {
    pub kpage: *mut u8,      // kernel VA of the frame
    pub upage: usize,        // user VA this frame is mapped to
    pub owner_tid: i32,      // thread that owns this frame
    pub pinned: bool,        // pinned frames can't be evicted (during syscall I/O)
}

// Safety: FrameEntry contains a raw pointer (kpage) but the frame table is only
// accessed from kernel context where interrupts are disabled or a single thread
// holds the lock. No concurrent mutable access is possible.
unsafe impl Send for FrameEntry {}
unsafe impl Sync for FrameEntry {}

/// Global frame table (Vec for clock-algorithm sequential access).
static mut FRAME_TABLE: Option<Vec<FrameEntry>> = None;

/// O(log n) index: kpage address → index in FRAME_TABLE.
/// Kept in sync with FRAME_TABLE for fast pin/unpin/free lookups.
static mut KPAGE_INDEX: Option<BTreeMap<usize, usize>> = None;

/// Clock hand for the second-chance (clock) eviction algorithm.
static mut CLOCK_HAND: usize = 0;

/// Maps (owner_tid, upage) -> swap_slot for pages that were evicted to swap.
static mut SWAP_MAP: Option<BTreeMap<(i32, usize), usize>> = None;

pub fn init() {
    unsafe {
        FRAME_TABLE = Some(Vec::new());
        KPAGE_INDEX = Some(BTreeMap::new());
        SWAP_MAP = Some(BTreeMap::new());
    }
}

/// Register a frame in the frame table and kpage index.
fn register_frame(kpage: *mut u8, upage: usize, tid: i32) {
    let entry = FrameEntry {
        kpage,
        upage,
        owner_tid: tid,
        pinned: false,
    };
    unsafe {
        if let Some(ref mut ft) = FRAME_TABLE {
            let idx = ft.len();
            ft.push(entry);
            if let Some(ref mut idx_map) = KPAGE_INDEX {
                idx_map.insert(kpage as usize, idx);
            }
        }
    }
}

/// Allocate a user frame. If memory is tight, evict a frame.
pub fn alloc_frame(upage: usize, tid: i32) -> *mut u8 {
    let kpage = palloc::get_page(PallocFlags::USER | PallocFlags::ZERO);
    if !kpage.is_null() {
        register_frame(kpage, upage, tid);
        return kpage;
    }

    // Out of memory -- evict a frame using the clock algorithm.
    match evict_frame() {
        Some(kpage) => {
            // Zero the reclaimed page.
            unsafe { core::ptr::write_bytes(kpage, 0, PGSIZE); }
            register_frame(kpage, upage, tid);
            kpage
        }
        None => core::ptr::null_mut(),
    }
}

/// Evict one frame using the clock (second-chance) algorithm.
///
/// Scans the frame table starting from `CLOCK_HAND`. For each frame
/// whose accessed bit is set, the bit is cleared and the hand advances
/// (second chance). The first frame found with accessed == 0 is evicted.
/// Dirty frames are written to swap first.
fn evict_frame() -> Option<*mut u8> {
    let ft = unsafe { (&raw mut FRAME_TABLE).as_mut().unwrap() }.as_mut()?;
    if ft.is_empty() {
        return None;
    }

    let len = ft.len();
    let clock = &raw mut CLOCK_HAND;

    // We scan at most 2*len entries (two full rotations) so every frame
    // gets at most one second chance.
    for _ in 0..2 * len {
        let hand = unsafe { *clock % len };
        unsafe { *clock = (*clock).wrapping_add(1); }

        let entry = &ft[hand];

        // Skip pinned frames (in use by a syscall).
        if entry.pinned {
            continue;
        }

        // We need the owner's page directory to inspect accessed/dirty bits.
        let pd = crate::thread::get_pagedir_by_tid(entry.owner_tid);
        if pd.is_null() {
            continue;
        }

        // Second-chance check: if accessed, clear and move on.
        if pagedir::is_accessed(pd, entry.upage) {
            pagedir::set_accessed(pd, entry.upage, false);
            continue;
        }

        // This frame was not recently accessed -- evict it.
        let kpage = entry.kpage;
        let upage = entry.upage;
        let owner_tid = entry.owner_tid;

        let is_dirty = pagedir::is_dirty(pd, upage);

        // Check the owner's SPT to determine eviction strategy.
        let owner_spt = crate::userprog::process::get_spt(owner_tid);
        let spt_entry_type = if !owner_spt.is_null() {
            let spt_ref = unsafe { &*owner_spt };
            spt_ref.find(upage).map(|e| (e.page_type, e.read_bytes))
        } else {
            None
        };

        // Determine if we need to swap out or can just discard.
        // Clean file-backed pages can be re-read from disk (no swap needed).
        let need_swap = match spt_entry_type {
            Some((crate::vm::page::PageType::Binary, read_bytes)) if !is_dirty && read_bytes > 0 => false,
            _ => is_dirty, // dirty pages always need swap; clean anonymous pages are zeroed
        };

        if need_swap {
            match swap::swap_out(kpage) {
                Some(slot) => {
                    // Update SPT if available, otherwise use legacy SWAP_MAP.
                    if !owner_spt.is_null() {
                        let spt_ref = unsafe { &mut *owner_spt };
                        if let Some(entry) = spt_ref.find_mut(upage) {
                            entry.location = crate::vm::page::PageLocation::InSwap(slot);
                        }
                    } else {
                        unsafe {
                            if let Some(ref mut map) = SWAP_MAP {
                                map.insert((owner_tid, upage), slot);
                            }
                        }
                    }
                }
                None => {
                    // Swap is full; skip this frame and try the next one.
                    continue;
                }
            }
        } else if !owner_spt.is_null() {
            // Clean eviction: update SPT location so fault handler knows where to reload.
            let spt_ref = unsafe { &mut *owner_spt };
            if let Some(entry) = spt_ref.find_mut(upage) {
                match entry.page_type {
                    crate::vm::page::PageType::Binary if entry.read_bytes > 0 => {
                        entry.location = crate::vm::page::PageLocation::OnDisk;
                    }
                    crate::vm::page::PageType::Anonymous => {
                        entry.location = crate::vm::page::PageLocation::Zero;
                    }
                    _ => {
                        entry.location = crate::vm::page::PageLocation::Zero;
                    }
                }
            }
        }

        // Unmap from the owner's page directory.
        pagedir::clear_page(pd, upage);

        // Remove from frame table and kpage index.
        unsafe {
            if let Some(ref mut idx_map) = KPAGE_INDEX {
                idx_map.remove(&(kpage as usize));
            }
        }
        ft.remove(hand);
        // Rebuild shifted indices after removal.
        unsafe {
            if let Some(ref mut idx_map) = KPAGE_INDEX {
                for (i, entry) in ft.iter().enumerate().skip(hand) {
                    idx_map.insert(entry.kpage as usize, i);
                }
            }
        }
        if unsafe { *clock } > 0 {
            unsafe { *clock -= 1; }
        }

        // Return the physical page for reuse (do NOT free it via palloc).
        return Some(kpage);
    }

    None // swap is full, all frames pinned, or all recently accessed
}

/// Look up a swap slot for a previously evicted page.
pub fn get_swap_slot(tid: i32, upage: usize) -> Option<usize> {
    let map = unsafe { (&raw const SWAP_MAP).as_ref().unwrap() };
    map.as_ref()?.get(&(tid, upage)).copied()
}

/// Remove a swap map entry after the page has been swapped back in.
pub fn remove_swap_entry(tid: i32, upage: usize) {
    unsafe {
        if let Some(ref mut map) = SWAP_MAP {
            map.remove(&(tid, upage));
        }
    }
}

/// Free a frame and remove from frame table.
pub fn free_frame(kpage: *mut u8) {
    unsafe {
        if let Some(ref mut ft) = FRAME_TABLE {
            if let Some(ref mut idx_map) = KPAGE_INDEX {
                if let Some(&idx) = idx_map.get(&(kpage as usize)) {
                    ft.swap_remove(idx);
                    idx_map.remove(&(kpage as usize));
                    // Update the index for the entry that was swapped into `idx`.
                    if idx < ft.len() {
                        idx_map.insert(ft[idx].kpage as usize, idx);
                    }
                }
            }
        }
    }
    palloc::free_page(kpage);
}

/// Pin the frame mapped to user page `upage` for thread `tid`.
/// While pinned, the frame cannot be evicted.
pub fn pin_upage(tid: i32, upage: usize) {
    unsafe {
        if let Some(ref mut ft) = FRAME_TABLE {
            if let Some(entry) = ft.iter_mut().find(|e| e.owner_tid == tid && e.upage == upage) {
                entry.pinned = true;
            }
        }
    }
}

/// Unpin the frame mapped to user page `upage` for thread `tid`.
pub fn unpin_upage(tid: i32, upage: usize) {
    unsafe {
        if let Some(ref mut ft) = FRAME_TABLE {
            if let Some(entry) = ft.iter_mut().find(|e| e.owner_tid == tid && e.upage == upage) {
                entry.pinned = false;
            }
        }
    }
}

