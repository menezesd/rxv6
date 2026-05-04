use alloc::vec::Vec;
use crate::devices::block::{self, BlockType, BLOCK_SECTOR_SIZE};
use crate::mem::vaddr::PGSIZE;

/// Number of disk sectors per page (4096 / 512 = 8).
const SECTORS_PER_PAGE: usize = PGSIZE / BLOCK_SECTOR_SIZE;

/// Swap slot allocator using a free-list bitmap.
static mut SWAP_BITMAP: Option<Vec<bool>> = None;
#[allow(dead_code)]
static mut SWAP_SLOT_COUNT: usize = 0;

/// Initialize swap. If a swap block device is available, set up the bitmap.
pub fn init() -> bool {
    if let Some(dev) = block::get_role(BlockType::Swap) {
        let slot_count = dev.size as usize / SECTORS_PER_PAGE;
        let mut bitmap = Vec::new();
        bitmap.resize(slot_count, false); // false = free
        unsafe {
            SWAP_SLOT_COUNT = slot_count;
            SWAP_BITMAP = Some(bitmap);
        }
        crate::kprintln!("swap: {} slots available", slot_count);
        true
    } else {
        // No swap device - that's OK, we just can't evict pages.
        unsafe { SWAP_BITMAP = Some(Vec::new()); }
        false
    }
}

/// Write a page to a swap slot. Returns the slot index.
#[allow(dead_code)]
pub fn swap_out(kpage: *const u8) -> Option<usize> {
    let slot = alloc_slot()?;
    let dev = block::get_role(BlockType::Swap)?;
    let base_sector = (slot * SECTORS_PER_PAGE) as u32;

    for i in 0..SECTORS_PER_PAGE {
        let offset = i * BLOCK_SECTOR_SIZE;
        let sector = base_sector + i as u32;
        // Heap-allocate sector buffer to avoid stack overflow
        let mut buf = alloc_sector_buf();
        unsafe {
            core::ptr::copy_nonoverlapping(
                kpage.add(offset),
                buf.as_mut_ptr(),
                BLOCK_SECTOR_SIZE,
            );
        }
        dev.write(sector, &buf);
    }
    Some(slot)
}

/// Read a page from a swap slot into kpage. Frees the slot.
#[allow(dead_code)]
pub fn swap_in(slot: usize, kpage: *mut u8) {
    let dev = block::get_role(BlockType::Swap)
        .expect("swap_in: no swap device");
    let base_sector = (slot * SECTORS_PER_PAGE) as u32;

    for i in 0..SECTORS_PER_PAGE {
        let offset = i * BLOCK_SECTOR_SIZE;
        let sector = base_sector + i as u32;
        let mut buf = alloc_sector_buf();
        dev.read(sector, &mut buf);
        unsafe {
            core::ptr::copy_nonoverlapping(
                buf.as_ptr(),
                kpage.add(offset),
                BLOCK_SECTOR_SIZE,
            );
        }
    }
    free_slot(slot);
}

fn alloc_slot() -> Option<usize> {
    unsafe {
        if let Some(ref mut bm) = SWAP_BITMAP {
            for (i, used) in bm.iter_mut().enumerate() {
                if !*used {
                    *used = true;
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Free a swap slot (public, for SPT cleanup on process exit).
pub fn free_slot_public(slot: usize) {
    free_slot(slot);
}

fn free_slot(slot: usize) {
    unsafe {
        if let Some(ref mut bm) = SWAP_BITMAP {
            if slot < bm.len() {
                bm[slot] = false;
            }
        }
    }
}

/// Allocate a zeroed sector buffer (delegates to shared block::sector_buf).
fn alloc_sector_buf() -> alloc::boxed::Box<[u8; BLOCK_SECTOR_SIZE]> {
    block::sector_buf()
}
