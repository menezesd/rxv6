//! Buffer cache (xv6-style).
//!
//! Caches disk sectors in memory using a fixed pool of buffers.
//! Uses LRU eviction. Each buffer can be locked for exclusive access.

use alloc::boxed::Box;
use crate::devices::block::{self, BlockSector, BlockType, BLOCK_SECTOR_SIZE};
use crate::sync::Lock;

const NBUF: usize = 30;

/// Flags for a buffer.
const B_VALID: u32 = 0x1; // data has been read from disk
const B_DIRTY: u32 = 0x2; // data needs to be written to disk

/// A cached disk sector.
pub struct Buf {
    pub flags: u32,
    pub sector: BlockSector,
    pub refcnt: u32,
    pub data: [u8; BLOCK_SECTOR_SIZE],
}

/// The buffer cache.
struct BCache {
    lock: Lock,
    bufs: [Buf; NBUF],
}

static mut BCACHE: Option<Box<BCache>> = None;

fn bcache() -> &'static mut BCache {
    static_mut!(BCACHE)
}

/// Initialize the buffer cache.
pub fn init() {
    // Heap-allocate to avoid huge stack usage
    let layout = core::alloc::Layout::new::<BCache>();
    unsafe {
        let ptr = alloc::alloc::alloc_zeroed(layout) as *mut BCache;
        assert!(!ptr.is_null(), "bio: alloc failed");
        (*ptr).lock = Lock::new();
        for buf in (*ptr).bufs.iter_mut() {
            buf.flags = 0;
            buf.sector = 0;
            buf.refcnt = 0;
        }
        BCACHE = Some(Box::from_raw(ptr));
    }
}

/// Get a locked buffer for `sector`. If already cached, return it.
/// Otherwise, evict the LRU unused buffer and return that.
pub fn bread(sector: BlockSector) -> &'static mut Buf {
    let bc = bcache();
    bc.lock.acquire();

    // Check if already cached
    for buf in bc.bufs.iter_mut() {
        if buf.sector == sector && (buf.flags & B_VALID) != 0 {
            buf.refcnt += 1;
            bc.lock.release();
            return unsafe { &mut *(buf as *mut Buf) };
        }
    }

    // Find an unreferenced buffer to evict (simple scan, not true LRU)
    for buf in bc.bufs.iter_mut() {
        if buf.refcnt == 0 {
            // Write back dirty buffer before reusing
            if (buf.flags & B_DIRTY) != 0 {
                write_sector(buf.sector, &buf.data);
                buf.flags &= !B_DIRTY;
            }

            buf.sector = sector;
            buf.flags = 0;
            buf.refcnt = 1;

            // Read from disk
            read_sector(sector, &mut buf.data);
            buf.flags |= B_VALID;

            bc.lock.release();
            return unsafe { &mut *(buf as *mut Buf) };
        }
    }

    bc.lock.release();
    panic!("bio: no free buffers");
}

/// Release a buffer. Decrements refcnt.
pub fn brelse(buf: &mut Buf) {
    let bc = bcache();
    bc.lock.acquire();
    buf.refcnt -= 1;
    bc.lock.release();
}

/// Mark a buffer as dirty (needs writeback).
pub fn bwrite(buf: &mut Buf) {
    buf.flags |= B_DIRTY;
    write_sector(buf.sector, &buf.data);
    buf.flags &= !B_DIRTY;
}

/// Flush all dirty buffers to disk.
pub fn sync() {
    let bc = bcache();
    bc.lock.acquire();
    for buf in bc.bufs.iter_mut() {
        if (buf.flags & (B_VALID | B_DIRTY)) == (B_VALID | B_DIRTY) {
            write_sector(buf.sector, &buf.data);
            buf.flags &= !B_DIRTY;
        }
    }
    bc.lock.release();
}

/// Invalidate a cached sector (e.g., after log recovery).
#[allow(dead_code)]
pub fn binvalidate(sector: BlockSector) {
    let bc = bcache();
    bc.lock.acquire();
    for buf in bc.bufs.iter_mut() {
        if buf.sector == sector {
            buf.flags = 0;
            buf.refcnt = 0;
        }
    }
    bc.lock.release();
}

// Low-level disk I/O helpers
fn read_sector(sector: BlockSector, data: &mut [u8; BLOCK_SECTOR_SIZE]) {
    if let Some(dev) = block::get_role(BlockType::FileSys) {
        dev.read(sector, data);
    }
}

fn write_sector(sector: BlockSector, data: &[u8; BLOCK_SECTOR_SIZE]) {
    if let Some(dev) = block::get_role(BlockType::FileSys) {
        dev.write(sector, data);
    }
}
