//! Free sector bitmap for the filesystem.
//!
//! Tracks which disk sectors are allocated using an in-memory bit vector.

use alloc::vec;
use alloc::vec::Vec;

/// Bit vector: each bit represents one sector (0 = free, 1 = used).
static mut FREE_MAP: Option<Vec<u8>> = None;
static mut SECTOR_COUNT: u32 = 0;
/// Guard against recursive flush (allocate → flush → write_at → allocate)
static mut IN_FLUSH: bool = false;

fn bitmap() -> &'static mut Vec<u8> {
    static_mut!(FREE_MAP)
}

fn bit_get(sector: u32) -> bool {
    let byte_idx = sector as usize / 8;
    let bit_idx = sector as usize % 8;
    let bm = bitmap();
    if byte_idx >= bm.len() {
        return false;
    }
    bm[byte_idx] & (1 << bit_idx) != 0
}

fn bit_set(sector: u32, val: bool) {
    let byte_idx = sector as usize / 8;
    let bit_idx = sector as usize % 8;
    let bm = bitmap();
    if byte_idx >= bm.len() {
        return;
    }
    if val {
        bm[byte_idx] |= 1 << bit_idx;
    } else {
        bm[byte_idx] &= !(1 << bit_idx);
    }
}

/// Initialize the free map for the given number of sectors.
/// Marks sectors 0 (free map inode) and 1 (root dir inode) as used.
pub fn init(sector_count: u32) {
    let byte_count = (sector_count as usize).div_ceil(8);
    let bm = vec![0u8; byte_count];
    unsafe {
        SECTOR_COUNT = sector_count;
        FREE_MAP = Some(bm);
    }
    // Mark inode sectors as used.
    bit_set(0, true); // free map inode
    bit_set(1, true); // root dir inode
}

/// Allocate `cnt` consecutive free sectors. Returns the first sector, or None.
/// Note: does NOT persist to disk (flush is too deep for 4KB stacks).
/// Caller should call `flush_if_safe()` after completing the high-level operation.
pub fn allocate(cnt: usize) -> Option<u32> {
    let total = unsafe { SECTOR_COUNT };
    if cnt == 0 {
        return None;
    }

    let mut start = 2u32;
    'outer: while start + cnt as u32 <= total {
        for i in 0..cnt {
            if bit_get(start + i as u32) {
                start = start + i as u32 + 1;
                continue 'outer;
            }
        }
        for i in 0..cnt {
            bit_set(start + i as u32, true);
        }
        return Some(start);
    }
    None
}

/// Release `cnt` sectors starting at `sector`.
pub fn release(sector: u32, cnt: usize) {
    for i in 0..cnt {
        bit_set(sector + i as u32, false);
    }
}

/// Flush free map to disk. Call from a thread with sufficient stack space.
#[allow(dead_code)]
pub fn flush_if_safe() {
    flush();
}

/// Write the free map to disk as data of the free_map inode (sector 0).
/// This creates the inode and writes the bitmap as its data.
pub fn create() {
    use super::inode;
    let bm_size = bitmap().len() as i32;
    // Create the free map inode at sector 0.
    // We need to be careful: the inode create will try to allocate data blocks
    // through us, which is fine since we're already initialized.
    inode::create(inode::FREE_MAP_SECTOR, bm_size, false);
    flush();
}

/// Read the free map from disk (via inode at sector 0).
pub fn open() {
    use super::inode;
    let _inode = inode::open(inode::FREE_MAP_SECTOR);
    let bm_size = bitmap().len();
    let mut data = vec![0u8; bm_size];
    let n = inode::read_at(inode::FREE_MAP_SECTOR, &mut data, bm_size as i32, 0);
    if n > 0 {
        let bm = bitmap();
        let copy_len = core::cmp::min(n as usize, bm.len());
        bm[..copy_len].copy_from_slice(&data[..copy_len]);
    }
}

/// Write the free map back to disk.
pub fn close() {
    flush();
    use super::inode;
    inode::close(inode::FREE_MAP_SECTOR);
}

/// Flush the bitmap data to the free_map inode on disk.
/// Guarded against recursive calls (allocate → flush → write_at → allocate).
fn flush() {
    unsafe {
        if IN_FLUSH { return; }
        IN_FLUSH = true;
    }
    {
        use super::inode;
        let bm = bitmap();
        let data: Vec<u8> = bm.clone();
        inode::write_at(inode::FREE_MAP_SECTOR, &data, data.len() as i32, 0);
    }
    unsafe { IN_FLUSH = false; }
}
