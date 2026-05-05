//! Write-ahead log for crash-consistent filesystem operations (xv6-style).
//!
//! All filesystem writes go through the log. A transaction groups
//! multiple sector writes, which are first written to a log area on disk,
//! then committed atomically by writing the log header. After commit,
//! the sectors are copied to their real locations ("install").
//!
//! On recovery after crash, if the log header indicates a committed
//! transaction, the logged sectors are re-installed.

use crate::devices::block::{self, BlockSector, BlockType};
use crate::sync::Lock;

/// Maximum sectors per transaction.
const LOGSIZE: usize = 10;

/// On-disk log header. Lives at the first sector of the log area.
#[repr(C)]
struct LogHeader {
    n: u32,                         // number of logged sectors
    sectors: [BlockSector; LOGSIZE], // destination sectors
}

/// In-memory log state.
struct Log {
    lock: Lock,
    start: BlockSector,   // first sector of log area on disk
    size: u32,            // total log area size in sectors
    outstanding: i32,     // active transactions (for nesting)
    committing: bool,
    header: LogHeader,
}

static mut LOG: Option<Log> = None;

fn log() -> &'static mut Log {
    static_mut!(LOG)
}

/// Initialize the log. `log_start` is the first sector of the log area,
/// `log_size` is the number of sectors reserved for the log.
pub fn init(log_start: BlockSector, log_size: u32) {
    unsafe {
        LOG = Some(Log {
            lock: Lock::new(),
            start: log_start,
            size: log_size,
            outstanding: 0,
            committing: false,
            header: LogHeader {
                n: 0,
                sectors: [0; LOGSIZE],
            },
        });
    }
    recover();
}

/// Begin a filesystem transaction.
pub fn begin_op() {
    let l = log();
    l.lock.acquire();
    l.outstanding += 1;
    l.lock.release();
}

/// End a filesystem transaction. Commits if this is the last outstanding op.
pub fn end_op() {
    let l = log();
    l.lock.acquire();
    l.outstanding -= 1;
    let do_commit = l.outstanding == 0;
    l.lock.release();

    if do_commit {
        commit();
    }
}

/// Record that `sector` should be written as part of the current transaction.
/// The actual data is read from the buffer cache at commit time.
pub fn log_write(sector: BlockSector) {
    let l = log();
    l.lock.acquire();
    let n = l.header.n as usize;
    if n >= LOGSIZE {
        l.lock.release();
        panic!("log: transaction too large");
    }
    // Check if sector is already in the log (absorb)
    for i in 0..n {
        if l.header.sectors[i] == sector {
            l.lock.release();
            return;
        }
    }
    l.header.sectors[n] = sector;
    l.header.n = (n + 1) as u32;
    l.lock.release();
}

/// Commit the current transaction: write log, write header, install, clear.
fn commit() {
    let l = log();
    if l.header.n == 0 {
        return;
    }

    // 1. Write modified sectors to log area
    write_log();
    // 2. Write header to disk (atomic commit point)
    write_head();
    // 3. Copy logged sectors to their real locations
    install_trans();
    // 4. Clear the log
    l.header.n = 0;
    write_head();
}

/// Recover from a crash by replaying committed transactions.
fn recover() {
    read_head();
    let l = log();
    if l.header.n > 0 {
        install_trans();
        l.header.n = 0;
        write_head();
    }
}

/// Write the in-memory log header to the log header sector on disk.
fn write_head() {
    let l = log();
    let mut buf = block::sector_buf();
    // Serialize header into buf
    let n = l.header.n;
    let ptr = buf.as_mut_ptr() as *mut u32;
    unsafe {
        *ptr = n;
        for i in 0..LOGSIZE {
            *ptr.add(1 + i) = l.header.sectors[i];
        }
    }
    if let Some(dev) = block::get_role(BlockType::FileSys) {
        dev.write(l.start, &buf);
    }
}

/// Read the log header from disk into memory.
fn read_head() {
    let l = log();
    let mut buf = block::sector_buf();
    if let Some(dev) = block::get_role(BlockType::FileSys) {
        dev.read(l.start, &mut buf);
    }
    let ptr = buf.as_ptr() as *const u32;
    unsafe {
        l.header.n = *ptr;
        for i in 0..(l.header.n as usize).min(LOGSIZE) {
            l.header.sectors[i] = *ptr.add(1 + i);
        }
    }
}

/// Copy modified sectors from their real locations to the log area.
fn write_log() {
    let l = log();
    let n = l.header.n as usize;
    let mut buf = block::sector_buf();
    for i in 0..n {
        let dst_sector = l.start + 1 + i as u32; // log body starts at start+1
        let src_sector = l.header.sectors[i];
        // Read from real location
        if let Some(dev) = block::get_role(BlockType::FileSys) {
            dev.read(src_sector, &mut buf);
        }
        // Write to log area
        if let Some(dev) = block::get_role(BlockType::FileSys) {
            dev.write(dst_sector, &buf);
        }
    }
}

/// Copy logged sectors from the log area to their real locations.
fn install_trans() {
    let l = log();
    let n = l.header.n as usize;
    let mut buf = block::sector_buf();
    for i in 0..n {
        let src_sector = l.start + 1 + i as u32;
        let dst_sector = l.header.sectors[i];
        // Read from log area
        if let Some(dev) = block::get_role(BlockType::FileSys) {
            dev.read(src_sector, &mut buf);
        }
        // Write to real location
        if let Some(dev) = block::get_role(BlockType::FileSys) {
            dev.write(dst_sector, &buf);
        }
    }
}
