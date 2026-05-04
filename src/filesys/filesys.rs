//! Top-level filesystem operations.
//!
//! Provides create/open/remove for files, with subdirectory path support.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use crate::devices::block::{self, BlockType};
use super::inode;
use super::free_map;
use super::directory::Dir;
use super::file::File;

/// Initialize the filesystem. If `format` is true, create a fresh filesystem.
pub fn init(format: bool) {
    let dev = block::get_role(BlockType::FileSys)
        .expect("filesys: no FileSys block device set");
    let sector_count = dev.size;

    inode::init();
    free_map::init(sector_count);

    if format {
        crate::kprintln!("filesys: formatting ({} sectors)...", sector_count);
        do_format(sector_count);
    } else {
        free_map::open();
    }
}

/// Resolve a path like "/foo/bar/baz" into (parent_dir, leaf_name).
/// Returns the parent directory and the final component name.
/// For absolute paths, starts at root. For relative paths (no leading /), starts at root too (no CWD yet).
fn resolve_path(path: &str) -> Option<(Box<Dir>, String)> {
    if path.is_empty() { return None; }

    let dir = Dir::open_root()?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        // Path is just "/" - return root with empty leaf
        return Some((dir, String::new()));
    }

    // Traverse all but the last component
    let mut current_dir = dir;
    for &part in &parts[..parts.len()-1] {
        let sector = match current_dir.lookup(part) {
            Some(s) => s,
            None => {
                current_dir.close_dir();
                return None;
            }
        };
        // Verify it's a directory
        if !inode::is_dir(sector) {
            current_dir.close_dir();
            return None;
        }
        current_dir.close_dir();
        // Open the subdirectory
        current_dir = Dir::open(sector)?;
    }

    let leaf = String::from(parts[parts.len()-1]);
    Some((current_dir, leaf))
}

/// Create a file with the given name and initial size.
/// Supports paths like "/foo/bar/file.txt".
pub fn create(path: &str, initial_size: i32) -> bool {
    let (mut parent, name) = match resolve_path(path) {
        Some(x) => x,
        None => return false,
    };
    if name.is_empty() { parent.close_dir(); return false; }

    // Allocate a sector for the new file's inode.
    let inode_sector = match free_map::allocate(1) {
        Some(s) => s,
        None => { parent.close_dir(); return false; }
    };

    // Create the inode on disk.
    if !inode::create(inode_sector, initial_size, false) {
        free_map::release(inode_sector, 1);
        parent.close_dir();
        return false;
    }

    let ok = parent.add(&name, inode_sector);
    parent.close_dir();

    if !ok {
        inode::remove(inode_sector);
        inode::close(inode_sector);
        free_map::release(inode_sector, 1);
        return false;
    }

    true
}

/// Open a file by path.
pub fn open(path: &str) -> Option<Box<File>> {
    let (parent, name) = resolve_path(path)?;
    if name.is_empty() {
        parent.close_dir();
        return None;
    }
    let sector = parent.lookup(&name);
    parent.close_dir();

    let sector = sector?;
    File::open(sector)
}

/// Remove a file by path.
#[allow(dead_code)]
pub fn remove(path: &str) -> bool {
    let (mut parent, name) = match resolve_path(path) {
        Some(x) => x,
        None => return false,
    };
    if name.is_empty() { parent.close_dir(); return false; }

    let ok = parent.remove(&name);
    parent.close_dir();
    ok
}

/// Create a directory at the given path.
pub fn mkdir(path: &str) -> bool {
    let (mut parent, name) = match resolve_path(path) {
        Some(x) => x,
        None => return false,
    };
    if name.is_empty() { parent.close_dir(); return false; }

    let inode_sector = match free_map::allocate(1) {
        Some(s) => s,
        None => { parent.close_dir(); return false; }
    };

    let parent_sector = parent.inode_sector;
    if !Dir::create(inode_sector, parent_sector) {
        free_map::release(inode_sector, 1);
        parent.close_dir();
        return false;
    }

    if !parent.add(&name, inode_sector) {
        inode::remove(inode_sector);
        inode::close(inode_sector);
        free_map::release(inode_sector, 1);
        parent.close_dir();
        return false;
    }

    parent.close_dir();
    true
}

/// Create a hard link: add `new_path` as a new name for the file at `old_path`.
pub fn link(old_path: &str, new_path: &str) -> bool {
    // Look up the existing file
    let (parent, name) = match resolve_path(old_path) {
        Some(x) => x,
        None => return false,
    };
    if name.is_empty() { parent.close_dir(); return false; }
    let sector = match parent.lookup(&name) {
        Some(s) => s,
        None => { parent.close_dir(); return false; }
    };
    parent.close_dir();

    // Can't link directories
    if inode::is_dir(sector) { return false; }

    // Add the new name pointing to the same inode sector
    let (mut new_parent, new_name) = match resolve_path(new_path) {
        Some(x) => x,
        None => return false,
    };
    if new_name.is_empty() { new_parent.close_dir(); return false; }

    // Bump the inode's open count (keeps it alive)
    inode::open(sector);
    let ok = new_parent.add(&new_name, sector);
    new_parent.close_dir();

    if !ok {
        inode::close(sector);
        return false;
    }
    true
}

/// Format the filesystem: create free map and root directory.
fn do_format(sector_count: u32) {
    let _ = sector_count;

    crate::kprintln!("  do_format: creating free map...");
    // Create the free map inode (sector 0) and write bitmap data.
    free_map::create();
    crate::kprintln!("  do_format: free map created.");

    // Create root directory with "." and ".." entries.
    crate::kprintln!("  do_format: creating root dir...");
    Dir::create(inode::ROOT_DIR_SECTOR, inode::ROOT_DIR_SECTOR);
    crate::kprintln!("  do_format: root dir created.");

    // Write the free map back.
    free_map::close();

    // Re-open free map for ongoing use.
    free_map::init(block::get_role(BlockType::FileSys)
        .expect("do_format: FileSys device disappeared").size);
    free_map::open();
}
