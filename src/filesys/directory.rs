//! Directory layer.
//!
//! Directories are files whose data is an array of DirEntry structs.

use alloc::boxed::Box;
use alloc::string::String;
use super::inode;

pub const NAME_MAX: usize = 14;

/// On-disk directory entry: 20 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
struct DirEntry {
    inode_sector: u32,          // 4
    name: [u8; NAME_MAX + 1],  // 15 (null-terminated)
    in_use: u8,                 // 1 (bool as u8 for repr(C))
}
// 4 + 15 + 1 = 20

const DIR_ENTRY_SIZE: usize = core::mem::size_of::<DirEntry>();
const _: () = assert!(DIR_ENTRY_SIZE == 20);

impl DirEntry {
    fn zeroed() -> Self {
        DirEntry {
            inode_sector: 0,
            name: [0; NAME_MAX + 1],
            in_use: 0,
        }
    }

    fn name_matches(&self, name: &str) -> bool {
        if self.in_use == 0 {
            return false;
        }
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(NAME_MAX + 1);
        if len != name.len() {
            return false;
        }
        &self.name[..len] == name.as_bytes()
    }
}

pub struct Dir {
    pub inode_sector: u32,
    pub pos: i32,
}

impl Dir {
    /// Open the root directory.
    pub fn open_root() -> Option<Box<Dir>> {
        let _inode = inode::open(inode::ROOT_DIR_SECTOR)?;
        Some(Box::new(Dir {
            inode_sector: inode::ROOT_DIR_SECTOR,
            pos: 0,
        }))
    }

    /// Create a new directory at `sector` with "." and ".." entries.
    #[allow(dead_code)]
    pub fn create(sector: u32, parent_sector: u32) -> bool {
        // Create the inode for this directory (initially enough for 2 entries).
        let initial_size = (DIR_ENTRY_SIZE * 2) as i32;
        if !inode::create(sector, initial_size, true) {
            return false;
        }

        // Write "." entry.
        let mut dot = DirEntry::zeroed();
        dot.inode_sector = sector;
        dot.in_use = 1;
        dot.name[0] = b'.';

        // Write ".." entry.
        let mut dotdot = DirEntry::zeroed();
        dotdot.inode_sector = parent_sector;
        dotdot.in_use = 1;
        dotdot.name[0] = b'.';
        dotdot.name[1] = b'.';

        let dot_bytes = entry_to_bytes(&dot);
        let dotdot_bytes = entry_to_bytes(&dotdot);

        inode::write_at(sector, &dot_bytes, DIR_ENTRY_SIZE as i32, 0);
        inode::write_at(sector, &dotdot_bytes, DIR_ENTRY_SIZE as i32, DIR_ENTRY_SIZE as i32);

        true
    }

    /// Look up a name in this directory. Returns the inode sector if found.
    pub fn lookup(&self, name: &str) -> Option<u32> {
        let len = inode::length(self.inode_sector);
        let entry_count = len / DIR_ENTRY_SIZE as i32;

        for i in 0..entry_count {
            let offset = i * DIR_ENTRY_SIZE as i32;
            let mut buf = [0u8; DIR_ENTRY_SIZE];
            let n = inode::read_at(self.inode_sector, &mut buf, DIR_ENTRY_SIZE as i32, offset);
            if n < DIR_ENTRY_SIZE as i32 {
                break;
            }
            let entry = bytes_to_entry(&buf);
            if entry.name_matches(name) {
                return Some(entry.inode_sector);
            }
        }
        None
    }

    /// Add a new entry mapping `name` to `inode_sector`.
    pub fn add(&mut self, name: &str, inode_sector: u32) -> bool {
        // Check for duplicate.
        if self.lookup(name).is_some() {
            return false;
        }

        if name.len() > NAME_MAX {
            return false;
        }

        // Find an unused slot or append.
        let len = inode::length(self.inode_sector);
        let entry_count = len / DIR_ENTRY_SIZE as i32;

        // Search for a free slot.
        for i in 0..entry_count {
            let offset = i * DIR_ENTRY_SIZE as i32;
            let mut buf = [0u8; DIR_ENTRY_SIZE];
            let n = inode::read_at(self.inode_sector, &mut buf, DIR_ENTRY_SIZE as i32, offset);
            if n < DIR_ENTRY_SIZE as i32 {
                break;
            }
            let entry = bytes_to_entry(&buf);
            if entry.in_use == 0 {
                // Reuse this slot.
                let new_entry = make_entry(name, inode_sector);
                let bytes = entry_to_bytes(&new_entry);
                inode::write_at(self.inode_sector, &bytes, DIR_ENTRY_SIZE as i32, offset);
                return true;
            }
        }

        // No free slot, append.
        let offset = entry_count * DIR_ENTRY_SIZE as i32;
        let new_entry = make_entry(name, inode_sector);
        let bytes = entry_to_bytes(&new_entry);
        let n = inode::write_at(self.inode_sector, &bytes, DIR_ENTRY_SIZE as i32, offset);
        n == DIR_ENTRY_SIZE as i32
    }

    /// Remove the entry with the given name.
    /// Refuses to remove non-empty directories.
    pub fn remove(&mut self, name: &str) -> bool {
        let len = inode::length(self.inode_sector);
        let entry_count = len / DIR_ENTRY_SIZE as i32;

        for i in 0..entry_count {
            let offset = i * DIR_ENTRY_SIZE as i32;
            let mut buf = [0u8; DIR_ENTRY_SIZE];
            let n = inode::read_at(self.inode_sector, &mut buf, DIR_ENTRY_SIZE as i32, offset);
            if n < DIR_ENTRY_SIZE as i32 {
                break;
            }
            let mut entry = bytes_to_entry(&buf);
            if entry.name_matches(name) {
                // If the target is a directory, check it's empty.
                if inode::is_dir(entry.inode_sector) {
                    let target_dir = Dir { inode_sector: entry.inode_sector, pos: 0 };
                    if !target_dir.is_empty() {
                        return false; // non-empty directory
                    }
                }
                // Mark the inode for removal.
                inode::remove(entry.inode_sector);
                // Clear the directory entry.
                entry.in_use = 0;
                let bytes = entry_to_bytes(&entry);
                inode::write_at(self.inode_sector, &bytes, DIR_ENTRY_SIZE as i32, offset);
                return true;
            }
        }
        false
    }

    /// Close this directory handle.
    pub fn close_dir(self) {
        inode::close(self.inode_sector);
    }

    /// Open a directory by inode sector.
    pub fn open(sector: u32) -> Option<Box<Dir>> {
        let _inode = inode::open(sector)?;
        Some(Box::new(Dir { inode_sector: sector, pos: 0 }))
    }

    /// Reopen (increment reference count).
    #[allow(dead_code)]
    pub fn reopen(&self) -> Option<Box<Dir>> {
        inode::reopen(self.inode_sector)?;
        Some(Box::new(Dir { inode_sector: self.inode_sector, pos: 0 }))
    }

    /// Read the next directory entry, skipping "." and "..".
    /// Returns Some(name) or None when exhausted.
    #[allow(dead_code)]
    pub fn readdir(&mut self) -> Option<String> {
        let len = inode::length(self.inode_sector);
        loop {
            if self.pos >= len { return None; }
            let mut buf = [0u8; DIR_ENTRY_SIZE];
            let n = inode::read_at(self.inode_sector, &mut buf, DIR_ENTRY_SIZE as i32, self.pos);
            self.pos += DIR_ENTRY_SIZE as i32;
            if n < DIR_ENTRY_SIZE as i32 { return None; }
            let entry = bytes_to_entry(&buf);
            if entry.in_use != 0 {
                let name_len = entry.name.iter().position(|&b| b == 0).unwrap_or(NAME_MAX);
                let name = core::str::from_utf8(&entry.name[..name_len]).ok()?;
                if name != "." && name != ".." {
                    return Some(String::from(name));
                }
            }
        }
    }

    /// Check if directory is empty (only "." and ".." are in use).
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let len = inode::length(self.inode_sector);
        let entry_count = len / DIR_ENTRY_SIZE as i32;
        for i in 0..entry_count {
            let offset = i * DIR_ENTRY_SIZE as i32;
            let mut buf = [0u8; DIR_ENTRY_SIZE];
            let n = inode::read_at(self.inode_sector, &mut buf, DIR_ENTRY_SIZE as i32, offset);
            if n < DIR_ENTRY_SIZE as i32 { break; }
            let entry = bytes_to_entry(&buf);
            if entry.in_use != 0 {
                let name_len = entry.name.iter().position(|&b| b == 0).unwrap_or(NAME_MAX);
                if let Ok(name) = core::str::from_utf8(&entry.name[..name_len]) {
                    if name != "." && name != ".." {
                        return false;
                    }
                }
            }
        }
        true
    }
}

fn make_entry(name: &str, inode_sector: u32) -> DirEntry {
    let mut entry = DirEntry::zeroed();
    entry.inode_sector = inode_sector;
    entry.in_use = 1;
    let len = name.len().min(NAME_MAX);
    entry.name[..len].copy_from_slice(&name.as_bytes()[..len]);
    entry
}

fn entry_to_bytes(entry: &DirEntry) -> [u8; DIR_ENTRY_SIZE] {
    let mut buf = [0u8; DIR_ENTRY_SIZE];
    // Safety: DirEntry is repr(C) and exactly DIR_ENTRY_SIZE (20) bytes.
    // The copy is bounded and both pointers are valid.
    unsafe {
        core::ptr::copy_nonoverlapping(
            entry as *const DirEntry as *const u8,
            buf.as_mut_ptr(),
            DIR_ENTRY_SIZE,
        );
    }
    buf
}

fn bytes_to_entry(buf: &[u8; DIR_ENTRY_SIZE]) -> DirEntry {
    // Safety: DirEntry is repr(C) with no padding requirements beyond u8/u32
    // alignment, and buf is exactly DIR_ENTRY_SIZE bytes.
    unsafe { core::ptr::read(buf.as_ptr() as *const DirEntry) }
}
