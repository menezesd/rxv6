//! Supplementary page table for demand paging.
//!
//! Each user process has an SPT that records where each virtual page's
//! data lives: in a file (not yet loaded), in physical memory, in swap,
//! or zero-filled. The page fault handler consults the SPT to load pages
//! on demand rather than eagerly at exec time.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use crate::mem::vaddr::PGSIZE;

/// Where a page's data currently lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageLocation {
    /// In physical memory (mapped in page directory).
    InMemory,
    /// Evicted to a swap slot.
    InSwap(usize),
    /// On disk (file-backed), not yet loaded.
    OnDisk,
    /// Zero-filled (anonymous), not yet allocated.
    Zero,
}

/// Type of page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageType {
    /// Executable segment (code/data from ELF).
    Binary,
    /// Memory-mapped file.
    #[allow(dead_code)]
    File,
    /// Anonymous page (stack, heap).
    Anonymous,
}

/// Entry in the supplementary page table.
pub struct SptEntry {
    pub vaddr: usize,
    pub location: PageLocation,
    pub page_type: PageType,
    pub writable: bool,
    /// Inode sector for file-backed pages (Binary/File).
    pub inode_sector: u32,
    /// Byte offset within the file where this page's data starts.
    pub file_offset: usize,
    /// Number of bytes to read from the file.
    pub read_bytes: usize,
    /// Offset within the page where file data starts (for non-page-aligned segments).
    pub page_ofs: usize,
}

/// Per-process supplementary page table.
pub struct SupplementaryPageTable {
    entries: BTreeMap<usize, Box<SptEntry>>,
}

impl SupplementaryPageTable {
    pub fn new() -> Self {
        SupplementaryPageTable {
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, entry: SptEntry) {
        let vaddr = entry.vaddr;
        self.entries.insert(vaddr, Box::new(entry));
    }

    pub fn find(&self, vaddr: usize) -> Option<&SptEntry> {
        let page = vaddr & !(PGSIZE - 1);
        self.entries.get(&page).map(|e| &**e)
    }

    pub fn find_mut(&mut self, vaddr: usize) -> Option<&mut SptEntry> {
        let page = vaddr & !(PGSIZE - 1);
        self.entries.get_mut(&page).map(|e| &mut **e)
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, vaddr: usize) -> Option<Box<SptEntry>> {
        let page = vaddr & !(PGSIZE - 1);
        self.entries.remove(&page)
    }

    /// Destroy all entries, freeing any swap slots held by evicted pages.
    pub fn destroy(&mut self) {
        for (_vaddr, entry) in self.entries.iter() {
            if let PageLocation::InSwap(slot) = entry.location {
                crate::vm::swap::free_slot_public(slot);
            }
        }
        self.entries.clear();
    }
}

/// Load a demand-paged page into a physical frame and install it in the
/// page directory. Shared by the page fault handler and syscall pre-faulting.
/// Returns true if the page was successfully loaded.
pub fn load_page(entry: &mut SptEntry, pd: *mut u32, page: usize, tid: i32) -> bool {
    match entry.location {
        PageLocation::OnDisk => {
            let kpage = crate::vm::frame::alloc_frame(page, tid);
            if kpage.is_null() { return false; }
            // Zero the entire page first (handles partial pages cleanly).
            unsafe { core::ptr::write_bytes(kpage, 0, PGSIZE); }
            if entry.read_bytes > 0 {
                let dst = unsafe { kpage.add(entry.page_ofs) };
                let buf = unsafe { core::slice::from_raw_parts_mut(dst, entry.read_bytes) };
                crate::filesys::inode::read_at(
                    entry.inode_sector, buf,
                    entry.read_bytes as i32, entry.file_offset as i32,
                );
            }
            if crate::userprog::pagedir::set_page(pd, page, kpage as usize, entry.writable) {
                entry.location = PageLocation::InMemory;
                true
            } else {
                crate::vm::frame::free_frame(kpage);
                false
            }
        }
        PageLocation::InSwap(slot) => {
            let kpage = crate::vm::frame::alloc_frame(page, tid);
            if kpage.is_null() { return false; }
            crate::vm::swap::swap_in(slot, kpage);
            if crate::userprog::pagedir::set_page(pd, page, kpage as usize, entry.writable) {
                entry.location = PageLocation::InMemory;
                true
            } else {
                crate::vm::frame::free_frame(kpage);
                false
            }
        }
        PageLocation::Zero => {
            let kpage = crate::vm::frame::alloc_frame(page, tid);
            if kpage.is_null() { return false; }
            if crate::userprog::pagedir::set_page(pd, page, kpage as usize, entry.writable) {
                entry.location = PageLocation::InMemory;
                true
            } else {
                crate::vm::frame::free_frame(kpage);
                false
            }
        }
        PageLocation::InMemory => true,
    }
}
