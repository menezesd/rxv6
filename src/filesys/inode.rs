//! Inode layer for the filesystem.
//!
//! On-disk inodes are 512 bytes (one sector). In-memory inodes track
//! open state and are deduplicated by sector number.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use crate::devices::block::{self, BlockSector, BlockType, BLOCK_SECTOR_SIZE};

pub const FREE_MAP_SECTOR: u32 = 0;
pub const ROOT_DIR_SECTOR: u32 = 1;
pub const DIRECT_PTR_CNT: usize = 12;
pub const PTRS_PER_BLOCK: usize = 128; // 512 / 4
pub const INODE_MAGIC: u32 = 0x494e4f44;

/// On-disk inode structure -- exactly 512 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InodeDisk {
    pub direct_blocks: [u32; DIRECT_PTR_CNT], // 48
    pub indirect_block: u32,                   // 4
    pub doubly_indirect_block: u32,            // 4
    pub length: i32,                           // 4
    pub write_end: i32,                        // 4
    pub magic: u32,                            // 4
    pub is_dir: u8,                            // 1 (bool as u8 for repr(C))
    pub is_symlink: u8,                        // 1
    pub _padding: [u8; 442],                   // pad to 512
}

// 48 + 4 + 4 + 4 + 4 + 4 + 1 + 1 + 442 = 512
const _: () = assert!(core::mem::size_of::<InodeDisk>() == 512);

impl InodeDisk {
    /// Allocate a zeroed InodeDisk directly on the heap.
    /// Avoids creating a 512-byte temporary on the 4KB thread stack.
    pub fn new_boxed() -> Box<Self> {
        let layout = core::alloc::Layout::new::<Self>();
        // Safety: layout is non-zero (512 bytes). alloc_zeroed returns a valid,
        // zeroed allocation or null. We panic on null since InodeDisk allocation
        // is critical to filesystem operation.
        unsafe {
            let ptr = alloc::alloc::alloc_zeroed(layout) as *mut Self;
            assert!(!ptr.is_null(), "InodeDisk::new_boxed: heap allocation failed");
            (*ptr).magic = INODE_MAGIC;
            Box::from_raw(ptr)
        }
    }
}

/// In-memory inode.
pub struct Inode {
    pub sector: BlockSector,
    pub open_cnt: i32,
    pub removed: bool,
    pub deny_write_cnt: i32,
    pub length: i32,
    pub is_dir: bool,
    #[allow(dead_code)]
    pub is_symlink: bool,
}

/// Allocate a zeroed sector buffer (delegates to shared block::sector_buf).
fn sector_buf() -> Box<[u8; BLOCK_SECTOR_SIZE]> {
    block::sector_buf()
}

/// Global map of open inodes, keyed by sector (O(log n) lookup).
static mut OPEN_INODES: Option<BTreeMap<BlockSector, Box<Inode>>> = None;

/// Access the global OPEN_INODES map via raw pointer.
fn open_inodes() -> &'static mut BTreeMap<BlockSector, Box<Inode>> {
    static_mut!(OPEN_INODES)
}

/// Index of the filesystem block device in the registry.
static mut FS_DEVICE_SET: bool = false;

fn fs_device() -> &'static mut block::BlockDevice {
    block::get_role(BlockType::FileSys).expect("filesys: no FileSys block device")
}

fn block_read(sector: u32, buf: &mut [u8; BLOCK_SECTOR_SIZE]) {
    fs_device().read(sector, buf);
}

fn block_write(sector: u32, buf: &[u8; BLOCK_SECTOR_SIZE]) {
    fs_device().write(sector, buf);
}

/// Initialize the inode layer.
pub fn init() {
    // Safety: called once during single-threaded boot (before scheduler starts).
    unsafe {
        OPEN_INODES = Some(BTreeMap::new());
        FS_DEVICE_SET = true;
    }
}

/// Read an InodeDisk from the given sector.
pub fn read_inode_disk(sector: BlockSector) -> Box<InodeDisk> {
    // Allocate directly on heap, then read into it, avoiding 512-byte stack temp.
    let mut disk = InodeDisk::new_boxed();
    // Safety: InodeDisk is repr(C), exactly 512 bytes (== BLOCK_SECTOR_SIZE),
    // so reinterpreting as a byte array for block I/O is valid.
    let buf_ptr = &mut *disk as *mut InodeDisk as *mut [u8; BLOCK_SECTOR_SIZE];
    block_read(sector, unsafe { &mut *buf_ptr });
    disk
}

/// Write an InodeDisk to the given sector.
pub fn write_inode_disk(sector: BlockSector, disk: &InodeDisk) {
    // Safety: InodeDisk is repr(C) and exactly BLOCK_SECTOR_SIZE bytes,
    // so reinterpreting as a byte array reference is valid.
    let buf = unsafe { &*(disk as *const InodeDisk as *const [u8; BLOCK_SECTOR_SIZE]) };
    block_write(sector, buf);
}

/// Create a new inode on disk at the given sector.
pub fn create(sector: BlockSector, length: i32, is_dir: bool) -> bool {
    let mut disk = InodeDisk::new_boxed();
    disk.length = length;
    disk.write_end = length;
    disk.is_dir = if is_dir { 1 } else { 0 };

    // Allocate data blocks for initial length.
    let sectors_needed = bytes_to_sectors(length);
    for i in 0..sectors_needed {
        let s = match super::free_map::allocate(1) {
            Some(s) => s,
            None => return false,
        };
        set_block_sector(&mut disk, i, s);
        // Zero out the newly allocated sector.
        let zero_buf = sector_buf();
        block_write(s, &zero_buf);
    }

    write_inode_disk(sector, &disk);
    true
}

/// Open an inode by sector. Returns a mutable reference.
/// Deduplicates by sector: if already open, just increments open_cnt.
pub fn open(sector: BlockSector) -> Option<&'static mut Inode> {
    // Safety: OPEN_INODES is only accessed from the single-threaded kernel
    // context. The 'static lifetime is valid because inodes persist in the
    // global BTreeMap until explicitly closed.
    let inodes = open_inodes();

    // O(log n) lookup by sector.
    if let Some(inode) = inodes.get_mut(&sector) {
        inode.open_cnt += 1;
        return Some(&mut **inode);
    }

    // Read from disk.
    let disk = read_inode_disk(sector);

    let inode = Box::new(Inode {
        sector,
        open_cnt: 1,
        removed: false,
        deny_write_cnt: 0,
        length: disk.length,
        is_dir: disk.is_dir != 0,
        is_symlink: disk.is_symlink != 0,
    });

    let inodes = open_inodes();
    inodes.insert(sector, inode);
    let entry = inodes.get_mut(&sector).expect("just inserted");
    Some(&mut **entry)
}

/// Reopen an already-open inode (increment open_cnt).
#[allow(dead_code)]
pub fn reopen(sector: BlockSector) -> Option<&'static mut Inode> {
    open(sector)
}

/// Close an inode (decrement open_cnt). If it reaches 0, remove from list.
/// If marked as removed, deallocate its blocks.
pub fn close(sector: BlockSector) {
    let inodes = open_inodes();
    if let Some(entry) = inodes.get_mut(&sector) {
        entry.open_cnt -= 1;
        if entry.open_cnt <= 0 {
            let inode = inodes.remove(&sector).unwrap();
            if inode.removed {
                // Free the inode sector itself.
                super::free_map::release(inode.sector, 1);
                // Free all data blocks.
                let disk = read_inode_disk(inode.sector);
                let cnt = bytes_to_sectors(inode.length);
                for i in 0..cnt {
                    if let Some(s) = get_block_sector_disk(&disk, i) {
                        super::free_map::release(s, 1);
                    }
                }
                // Free indirect block if used.
                if disk.indirect_block != 0 {
                    super::free_map::release(disk.indirect_block, 1);
                }
                // Free doubly indirect blocks if used.
                if disk.doubly_indirect_block != 0 {
                    let mut buf = sector_buf();
                    block_read(disk.doubly_indirect_block, &mut buf);
                    // Safety: buf is 512 bytes = 128 u32s = PTRS_PER_BLOCK entries.
                    let ptrs = buf.as_ptr() as *const u32;
                    for j in 0..PTRS_PER_BLOCK {
                        let p = unsafe { *ptrs.add(j) };
                        if p != 0 {
                            super::free_map::release(p, 1);
                        }
                    }
                    super::free_map::release(disk.doubly_indirect_block, 1);
                }
            }
        }
    }
}

/// Mark an inode for removal (it will be freed when the last handle closes).
pub fn remove(sector: BlockSector) {
    if let Some(inode) = open_inodes().get_mut(&sector) {
        inode.removed = true;
    }
}

/// Truncate file to zero length.
pub fn truncate(sector: BlockSector) {
    if let Some(inode) = open_inodes().get_mut(&sector) {
        inode.length = 0;
    }
    let mut disk = read_inode_disk(sector);
    disk.length = 0;
    write_inode_disk(sector, &disk);
}

/// Return the length of the inode in bytes.
pub fn length(sector: BlockSector) -> i32 {
    if let Some(inode) = open_inodes().get(&sector) {
        return inode.length;
    }
    // Not open, read from disk.
    let disk = read_inode_disk(sector);
    disk.length
}

/// Check if an inode is a directory.
pub fn is_dir(sector: BlockSector) -> bool {
    if let Some(inode) = open_inodes().get(&sector) {
        return inode.is_dir;
    }
    let disk = read_inode_disk(sector);
    disk.is_dir != 0
}

/// Read `size` bytes from inode starting at `offset`. Returns bytes read.
pub fn read_at(sector: BlockSector, buf: &mut [u8], size: i32, offset: i32) -> i32 {
    let inode_length = length(sector);
    if offset >= inode_length {
        return 0;
    }

    let disk = read_inode_disk(sector);
    let mut bytes_read = 0i32;
    let mut off = offset;
    let mut buf_pos = 0usize;

    while bytes_read < size && off < inode_length {
        let sector_idx = (off / BLOCK_SECTOR_SIZE as i32) as u32;
        let sector_ofs = (off % BLOCK_SECTOR_SIZE as i32) as usize;
        let remaining_in_inode = inode_length - off;
        let remaining_wanted = size - bytes_read;
        let chunk_size = core::cmp::min(
            BLOCK_SECTOR_SIZE - sector_ofs,
            core::cmp::min(remaining_in_inode as usize, remaining_wanted as usize),
        );

        if let Some(data_sector) = get_block_sector_disk(&disk, sector_idx) {
            if sector_ofs == 0 && chunk_size == BLOCK_SECTOR_SIZE {
                // Read directly into output buffer (needs a temp since block_read needs [u8; 512]).
                let mut tmp = sector_buf();
                block_read(data_sector, &mut tmp);
                buf[buf_pos..buf_pos + chunk_size].copy_from_slice(&tmp[..chunk_size]);
            } else {
                // Bounce buffer for partial sector.
                let mut bounce = sector_buf();
                block_read(data_sector, &mut bounce);
                buf[buf_pos..buf_pos + chunk_size]
                    .copy_from_slice(&bounce[sector_ofs..sector_ofs + chunk_size]);
            }
        } else {
            // Block not allocated, read as zeros.
            for b in &mut buf[buf_pos..buf_pos + chunk_size] {
                *b = 0;
            }
        }

        bytes_read += chunk_size as i32;
        off += chunk_size as i32;
        buf_pos += chunk_size;
    }

    bytes_read
}

/// Deny writes to the inode with the given sector.
pub fn deny_write(sector: BlockSector) {
    if let Some(inode) = open_inodes().get_mut(&sector) {
        inode.deny_write_cnt += 1;
    }
}

/// Allow writes to the inode with the given sector.
pub fn allow_write(sector: BlockSector) {
    if let Some(inode) = open_inodes().get_mut(&sector) {
        if inode.deny_write_cnt > 0 {
            inode.deny_write_cnt -= 1;
        }
    }
}

/// Write `size` bytes to inode starting at `offset`. Auto-extends file. Returns bytes written.
pub fn write_at(sector: BlockSector, buf: &[u8], size: i32, offset: i32) -> i32 {
    // Check if writes are denied
    if let Some(inode) = open_inodes().get(&sector) {
        if inode.deny_write_cnt > 0 {
            return 0;
        }
    }

    let mut disk = *read_inode_disk(sector);
    let mut bytes_written = 0i32;
    let mut off = offset;
    let mut buf_pos = 0usize;

    while bytes_written < size {
        let sector_idx = (off / BLOCK_SECTOR_SIZE as i32) as u32;
        let sector_ofs = (off % BLOCK_SECTOR_SIZE as i32) as usize;
        let remaining_wanted = (size - bytes_written) as usize;
        let chunk_size = core::cmp::min(BLOCK_SECTOR_SIZE - sector_ofs, remaining_wanted);

        // Ensure this block is allocated (auto-extend).
        let data_sector = match get_block_sector_disk(&disk, sector_idx) {
            Some(s) if s != 0 => s,
            _ => {
                // Need to allocate a new block.
                if let Some(new_sec) = allocate_block_for(&mut disk, sector_idx) {
                    // Zero the new sector.
                    let zero_buf = sector_buf();
                    block_write(new_sec, &zero_buf);
                    new_sec
                } else {
                    break; // out of space
                }
            }
        };

        if sector_ofs == 0 && chunk_size == BLOCK_SECTOR_SIZE {
            // Write full sector.
            let mut tmp = sector_buf();
            tmp[..chunk_size].copy_from_slice(&buf[buf_pos..buf_pos + chunk_size]);
            block_write(data_sector, &tmp);
        } else {
            // Read-modify-write for partial sector.
            let mut bounce = sector_buf();
            block_read(data_sector, &mut bounce);
            bounce[sector_ofs..sector_ofs + chunk_size]
                .copy_from_slice(&buf[buf_pos..buf_pos + chunk_size]);
            block_write(data_sector, &bounce);
        }

        bytes_written += chunk_size as i32;
        off += chunk_size as i32;
        buf_pos += chunk_size;
    }

    // Update length if we extended the file.
    let new_end = offset + bytes_written;
    if new_end > disk.length {
        disk.length = new_end;
        disk.write_end = new_end;
    }
    write_inode_disk(sector, &disk);

    // Update in-memory inode length.
    if let Some(inode) = open_inodes().get_mut(&sector) {
        inode.length = disk.length;
    }

    bytes_written
}

// ---- Block indexing ----

fn bytes_to_sectors(bytes: i32) -> u32 {
    if bytes <= 0 {
        return 0;
    }
    (bytes as u32).div_ceil(BLOCK_SECTOR_SIZE as u32)
}

/// Get the physical sector for a logical block index using 3-level indexing.
///
/// Uses direct → indirect → doubly-indirect block lookup (like ext2).
/// Safety: pointer arithmetic on sector buffers is bounded by PTRS_PER_BLOCK (128).
fn get_block_sector_disk(disk: &InodeDisk, block_idx: u32) -> Option<u32> {
    if (block_idx as usize) < DIRECT_PTR_CNT {
        // Direct block.
        let s = disk.direct_blocks[block_idx as usize];
        if s == 0 { None } else { Some(s) }
    } else if (block_idx as usize) < DIRECT_PTR_CNT + PTRS_PER_BLOCK {
        // Single indirect.
        if disk.indirect_block == 0 {
            return None;
        }
        let idx = block_idx as usize - DIRECT_PTR_CNT;
        let mut buf = sector_buf();
        block_read(disk.indirect_block, &mut buf);
        let ptrs = buf.as_ptr() as *const u32;
        let s = unsafe { *ptrs.add(idx) };
        if s == 0 { None } else { Some(s) }
    } else {
        // Doubly indirect.
        if disk.doubly_indirect_block == 0 {
            return None;
        }
        let rel = block_idx as usize - DIRECT_PTR_CNT - PTRS_PER_BLOCK;
        let outer_idx = rel / PTRS_PER_BLOCK;
        let inner_idx = rel % PTRS_PER_BLOCK;

        let mut buf = sector_buf();
        block_read(disk.doubly_indirect_block, &mut buf);
        let outer_ptrs = buf.as_ptr() as *const u32;
        let inner_sector = unsafe { *outer_ptrs.add(outer_idx) };
        if inner_sector == 0 {
            return None;
        }

        let mut buf2 = sector_buf();
        block_read(inner_sector, &mut buf2);
        let inner_ptrs = buf2.as_ptr() as *const u32;
        let s = unsafe { *inner_ptrs.add(inner_idx) };
        if s == 0 { None } else { Some(s) }
    }
}

/// Set the physical sector for a logical block index in an InodeDisk (no allocation of indirect blocks).
/// Used during inode creation when we already have the sector allocated.
fn set_block_sector(disk: &mut InodeDisk, block_idx: u32, sector: u32) {
    if (block_idx as usize) < DIRECT_PTR_CNT {
        disk.direct_blocks[block_idx as usize] = sector;
    }
    // For initial creation, we only support direct blocks.
    // Indirect blocks are handled by allocate_block_for during writes.
}

/// Allocate a data sector for a logical block index, updating the InodeDisk.
fn allocate_block_for(disk: &mut InodeDisk, block_idx: u32) -> Option<u32> {
    let new_sector = super::free_map::allocate(1)?;

    if (block_idx as usize) < DIRECT_PTR_CNT {
        disk.direct_blocks[block_idx as usize] = new_sector;
    } else if (block_idx as usize) < DIRECT_PTR_CNT + PTRS_PER_BLOCK {
        // Single indirect.
        if disk.indirect_block == 0 {
            disk.indirect_block = super::free_map::allocate(1)?;
            let zero = sector_buf();
            block_write(disk.indirect_block, &zero);
        }
        let idx = block_idx as usize - DIRECT_PTR_CNT;
        let mut buf = sector_buf();
        block_read(disk.indirect_block, &mut buf);
        unsafe {
            let ptrs = buf.as_mut_ptr() as *mut u32;
            *ptrs.add(idx) = new_sector;
        }
        block_write(disk.indirect_block, &buf);
    } else {
        // Doubly indirect.
        if disk.doubly_indirect_block == 0 {
            disk.doubly_indirect_block = super::free_map::allocate(1)?;
            let zero = sector_buf();
            block_write(disk.doubly_indirect_block, &zero);
        }
        let rel = block_idx as usize - DIRECT_PTR_CNT - PTRS_PER_BLOCK;
        let outer_idx = rel / PTRS_PER_BLOCK;
        let inner_idx = rel % PTRS_PER_BLOCK;

        let mut outer_buf = sector_buf();
        block_read(disk.doubly_indirect_block, &mut outer_buf);
        let inner_sector = unsafe {
            let ptrs = outer_buf.as_mut_ptr() as *mut u32;
            let s = *ptrs.add(outer_idx);
            if s == 0 {
                let new_inner = super::free_map::allocate(1)?;
                *ptrs.add(outer_idx) = new_inner;
                block_write(disk.doubly_indirect_block, &outer_buf);
                let zero = sector_buf();
                block_write(new_inner, &zero);
                new_inner
            } else {
                s
            }
        };

        let mut inner_buf = sector_buf();
        block_read(inner_sector, &mut inner_buf);
        unsafe {
            let ptrs = inner_buf.as_mut_ptr() as *mut u32;
            *ptrs.add(inner_idx) = new_sector;
        }
        block_write(inner_sector, &inner_buf);
    }

    Some(new_sector)
}
