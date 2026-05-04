//! File handle layer.
//!
//! Wraps an inode sector with a current position for sequential I/O.

use alloc::boxed::Box;
use super::inode;

pub struct File {
    pub inode_sector: u32,
    pub pos: i32,
    pub deny_write: bool,
}

impl File {
    /// Open a file on the given inode sector.
    pub fn open(inode_sector: u32) -> Option<Box<File>> {
        let _inode = inode::open(inode_sector)?;
        Some(Box::new(File {
            inode_sector,
            pos: 0,
            deny_write: false,
        }))
    }

    /// Close this file handle.
    pub fn close_file(self) {
        if self.deny_write {
            inode::allow_write(self.inode_sector);
        }
        inode::close(self.inode_sector);
    }

    /// Deny writes to the underlying inode.
    #[allow(dead_code)]
    pub fn deny_write(&mut self) {
        inode::deny_write(self.inode_sector);
        self.deny_write = true;
    }

    /// Read up to `size` bytes into `buf` at the current position.
    pub fn read(&mut self, buf: &mut [u8], size: i32) -> i32 {
        let n = inode::read_at(self.inode_sector, buf, size, self.pos);
        self.pos += n;
        n
    }

    /// Write up to `size` bytes from `buf` at the current position.
    pub fn write(&mut self, buf: &[u8], size: i32) -> i32 {
        let n = inode::write_at(self.inode_sector, buf, size, self.pos);
        self.pos += n;
        n
    }

    /// Seek to an absolute position.
    pub fn seek(&mut self, pos: i32) {
        self.pos = pos;
    }

    /// Return current position.
    #[allow(dead_code)]
    pub fn tell(&self) -> i32 {
        self.pos
    }

    /// Return file length.
    #[allow(dead_code)]
    pub fn length(&self) -> i32 {
        inode::length(self.inode_sector)
    }
}
