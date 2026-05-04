/// mkdisk -- create a RustOS filesystem disk image containing ELF programs.
///
/// Usage: mkdisk <output.img> <size_mb> [file1.elf file2.elf ...]

use std::env;
use std::fs;
use std::path::Path;
use std::process;

const SECTOR_SIZE: usize = 512;
const DIRECT_PTR_CNT: usize = 12;
const PTRS_PER_BLOCK: usize = 128; // 512 / 4
const INODE_MAGIC: u32 = 0x494e4f44;
const FREE_MAP_SECTOR: u32 = 0;
const ROOT_DIR_SECTOR: u32 = 1;
const NAME_MAX: usize = 14;
const DIR_ENTRY_SIZE: usize = 20;

// ---- On-disk structures ----------------------------------------------------

/// InodeDisk: 512 bytes, matches kernel's repr(C) layout exactly.
#[repr(C)]
#[derive(Clone, Copy)]
struct InodeDisk {
    direct_blocks: [u32; DIRECT_PTR_CNT], // 48
    indirect_block: u32,                   // 4
    doubly_indirect_block: u32,            // 4
    length: i32,                           // 4
    write_end: i32,                        // 4
    magic: u32,                            // 4
    is_dir: u8,                            // 1
    is_symlink: u8,                        // 1
    _padding: [u8; 442],                   // 442
}
const _: () = assert!(std::mem::size_of::<InodeDisk>() == 512);

impl InodeDisk {
    fn new() -> Self {
        InodeDisk {
            direct_blocks: [0; DIRECT_PTR_CNT],
            indirect_block: 0,
            doubly_indirect_block: 0,
            length: 0,
            write_end: 0,
            magic: INODE_MAGIC,
            is_dir: 0,
            is_symlink: 0,
            _padding: [0; 442],
        }
    }

    fn to_bytes(&self) -> [u8; SECTOR_SIZE] {
        unsafe { std::mem::transmute(*self) }
    }
}

/// DirEntry: 20 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
struct DirEntry {
    inode_sector: u32,
    name: [u8; NAME_MAX + 1], // 15 bytes, null-terminated
    in_use: u8,
}
const _: () = assert!(std::mem::size_of::<DirEntry>() == 20);

impl DirEntry {
    fn new(name: &str, sector: u32) -> Self {
        let mut entry = DirEntry {
            inode_sector: sector,
            name: [0; NAME_MAX + 1],
            in_use: 1,
        };
        let len = name.len().min(NAME_MAX);
        entry.name[..len].copy_from_slice(&name.as_bytes()[..len]);
        entry
    }

    fn to_bytes(&self) -> [u8; DIR_ENTRY_SIZE] {
        unsafe { std::mem::transmute(*self) }
    }
}

// ---- Disk image builder ----------------------------------------------------

struct DiskBuilder {
    data: Vec<u8>,
    sector_count: u32,
    next_free: u32,
    used: Vec<bool>, // per-sector usage bitmap
}

impl DiskBuilder {
    fn new(size_mb: usize) -> Self {
        let sector_count = (size_mb * 1024 * 1024 / SECTOR_SIZE) as u32;
        let data = vec![0u8; sector_count as usize * SECTOR_SIZE];
        let mut used = vec![false; sector_count as usize];
        // Reserve sectors 0 (free map inode) and 1 (root dir inode)
        used[0] = true;
        used[1] = true;
        DiskBuilder {
            data,
            sector_count,
            next_free: 2,
            used,
        }
    }

    /// Allocate one sector, return its number.
    fn alloc_sector(&mut self) -> u32 {
        let s = self.next_free;
        assert!(
            (s as usize) < self.used.len(),
            "disk full: tried to allocate sector {} but only {} available",
            s,
            self.sector_count
        );
        self.used[s as usize] = true;
        self.next_free += 1;
        s
    }

    /// Allocate N consecutive sectors, return the first.
    fn alloc_sectors(&mut self, n: u32) -> u32 {
        let first = self.next_free;
        for _ in 0..n {
            self.alloc_sector();
        }
        first
    }

    /// Write bytes into a specific sector.
    fn write_sector_bytes(&mut self, sector: u32, bytes: &[u8]) {
        let off = sector as usize * SECTOR_SIZE;
        let len = bytes.len().min(SECTOR_SIZE);
        self.data[off..off + len].copy_from_slice(&bytes[..len]);
    }

    /// Write a file's data to disk, setting up its inode with direct and
    /// indirect blocks as needed. Returns the inode sector.
    fn write_file(&mut self, name: &str, file_data: &[u8]) -> u32 {
        let inode_sector = self.alloc_sector();
        let data_sector_count = (file_data.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;

        // Allocate all data sectors consecutively
        let first_data_sector = self.alloc_sectors(data_sector_count as u32);

        // Write file data
        for i in 0..data_sector_count {
            let offset = i * SECTOR_SIZE;
            let end = (offset + SECTOR_SIZE).min(file_data.len());
            let sector = first_data_sector + i as u32;
            let dst_off = sector as usize * SECTOR_SIZE;
            self.data[dst_off..dst_off + (end - offset)]
                .copy_from_slice(&file_data[offset..end]);
        }

        // Build the inode
        let mut inode = InodeDisk::new();
        inode.length = file_data.len() as i32;
        inode.write_end = file_data.len() as i32;

        // Set up block pointers
        // Direct blocks: indices 0..11
        let direct_count = data_sector_count.min(DIRECT_PTR_CNT);
        for i in 0..direct_count {
            inode.direct_blocks[i] = first_data_sector + i as u32;
        }

        // Indirect block: indices 12..139 (128 pointers)
        if data_sector_count > DIRECT_PTR_CNT {
            let indirect_sector = self.alloc_sector();
            inode.indirect_block = indirect_sector;

            let mut indirect_data = [0u8; SECTOR_SIZE];
            let indirect_count =
                (data_sector_count - DIRECT_PTR_CNT).min(PTRS_PER_BLOCK);
            for i in 0..indirect_count {
                let data_sec = first_data_sector + (DIRECT_PTR_CNT + i) as u32;
                let bytes = data_sec.to_le_bytes();
                indirect_data[i * 4..i * 4 + 4].copy_from_slice(&bytes);
            }
            self.write_sector_bytes(indirect_sector, &indirect_data);
        }

        // Doubly indirect: indices 140+
        if data_sector_count > DIRECT_PTR_CNT + PTRS_PER_BLOCK {
            let doubly_sector = self.alloc_sector();
            inode.doubly_indirect_block = doubly_sector;

            let remaining = data_sector_count - DIRECT_PTR_CNT - PTRS_PER_BLOCK;
            let outer_count = (remaining + PTRS_PER_BLOCK - 1) / PTRS_PER_BLOCK;

            let mut outer_data = [0u8; SECTOR_SIZE];
            for outer_i in 0..outer_count {
                let inner_sector = self.alloc_sector();
                let bytes = inner_sector.to_le_bytes();
                outer_data[outer_i * 4..outer_i * 4 + 4].copy_from_slice(&bytes);

                let mut inner_data = [0u8; SECTOR_SIZE];
                let base = DIRECT_PTR_CNT + PTRS_PER_BLOCK + outer_i * PTRS_PER_BLOCK;
                let inner_count = (data_sector_count - base).min(PTRS_PER_BLOCK);
                for inner_i in 0..inner_count {
                    let data_sec = first_data_sector + (base + inner_i) as u32;
                    let b = data_sec.to_le_bytes();
                    inner_data[inner_i * 4..inner_i * 4 + 4].copy_from_slice(&b);
                }
                self.write_sector_bytes(inner_sector, &inner_data);
            }
            self.write_sector_bytes(doubly_sector, &outer_data);
        }

        // Write the inode
        self.write_sector_bytes(inode_sector, &inode.to_bytes());

        eprintln!(
            "  {} -> inode={}, data=[{}..{}), {} sectors, {} bytes",
            name,
            inode_sector,
            first_data_sector,
            first_data_sector + data_sector_count as u32,
            data_sector_count,
            file_data.len()
        );

        inode_sector
    }

    /// Write the root directory containing "." / ".." and file entries.
    fn write_root_dir(&mut self, file_entries: &[(String, u32)]) {
        // Build directory entry list: ".", "..", then each file
        let mut entries: Vec<DirEntry> = Vec::new();

        // "." points to root dir inode (sector 1)
        entries.push(DirEntry::new(".", ROOT_DIR_SECTOR));
        // ".." also points to root (root is its own parent)
        entries.push(DirEntry::new("..", ROOT_DIR_SECTOR));

        for (name, sector) in file_entries {
            entries.push(DirEntry::new(name, *sector));
        }

        // Serialize all entries into contiguous bytes
        let mut dir_data = Vec::new();
        for e in &entries {
            dir_data.extend_from_slice(&e.to_bytes());
        }

        // Allocate sectors for the directory data
        let dir_sectors = (dir_data.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;
        let first_dir_data = self.alloc_sectors(dir_sectors as u32);

        // Write directory data
        for i in 0..dir_sectors {
            let offset = i * SECTOR_SIZE;
            let end = (offset + SECTOR_SIZE).min(dir_data.len());
            let sector = first_dir_data + i as u32;
            let dst_off = sector as usize * SECTOR_SIZE;
            self.data[dst_off..dst_off + (end - offset)]
                .copy_from_slice(&dir_data[offset..end]);
        }

        // Build root directory inode
        let mut inode = InodeDisk::new();
        inode.length = dir_data.len() as i32;
        inode.write_end = dir_data.len() as i32;
        inode.is_dir = 1;

        for i in 0..dir_sectors.min(DIRECT_PTR_CNT) {
            inode.direct_blocks[i] = first_dir_data + i as u32;
        }
        // Root dir unlikely to exceed 12 sectors (306 entries), but handle it
        if dir_sectors > DIRECT_PTR_CNT {
            let indirect_sector = self.alloc_sector();
            inode.indirect_block = indirect_sector;
            let mut indirect_data = [0u8; SECTOR_SIZE];
            let count = (dir_sectors - DIRECT_PTR_CNT).min(PTRS_PER_BLOCK);
            for i in 0..count {
                let s = first_dir_data + (DIRECT_PTR_CNT + i) as u32;
                indirect_data[i * 4..i * 4 + 4].copy_from_slice(&s.to_le_bytes());
            }
            self.write_sector_bytes(indirect_sector, &indirect_data);
        }

        // Write root dir inode at sector 1
        self.write_sector_bytes(ROOT_DIR_SECTOR, &inode.to_bytes());

        eprintln!(
            "  root dir: {} entries, {} bytes, data at sectors [{}..{})",
            entries.len(),
            dir_data.len(),
            first_dir_data,
            first_dir_data + dir_sectors as u32,
        );
    }

    /// Write the free map inode (sector 0) and its bitmap data.
    fn write_free_map(&mut self) {
        // Build the bitmap: 1 bit per sector, LSB first within each byte
        let bitmap_bytes = (self.sector_count as usize + 7) / 8;
        let mut bitmap = vec![0u8; bitmap_bytes];
        for (i, &is_used) in self.used.iter().enumerate() {
            if is_used {
                bitmap[i / 8] |= 1 << (i % 8);
            }
        }

        // Allocate sectors for the bitmap data
        let bm_sectors = (bitmap.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;
        let first_bm_data = self.alloc_sectors(bm_sectors as u32);

        // Mark the bitmap data sectors as used too (they weren't yet)
        // We need to re-generate the bitmap after this allocation
        // Actually, alloc_sectors already marked them. Re-generate bitmap.
        let mut bitmap = vec![0u8; bitmap_bytes];
        for (i, &is_used) in self.used.iter().enumerate() {
            if is_used {
                bitmap[i / 8] |= 1 << (i % 8);
            }
        }

        // Write bitmap data
        for i in 0..bm_sectors {
            let offset = i * SECTOR_SIZE;
            let end = (offset + SECTOR_SIZE).min(bitmap.len());
            let sector = first_bm_data + i as u32;
            let dst_off = sector as usize * SECTOR_SIZE;
            self.data[dst_off..dst_off + (end - offset)]
                .copy_from_slice(&bitmap[offset..end]);
        }

        // Build free map inode
        let mut inode = InodeDisk::new();
        inode.length = bitmap.len() as i32;
        inode.write_end = bitmap.len() as i32;
        inode.is_dir = 0;

        for i in 0..bm_sectors.min(DIRECT_PTR_CNT) {
            inode.direct_blocks[i] = first_bm_data + i as u32;
        }
        if bm_sectors > DIRECT_PTR_CNT {
            let indirect_sector = self.alloc_sector();
            inode.indirect_block = indirect_sector;
            // Regenerate bitmap again since we just allocated
            let mut bitmap2 = vec![0u8; bitmap_bytes];
            for (i, &is_used) in self.used.iter().enumerate() {
                if is_used {
                    bitmap2[i / 8] |= 1 << (i % 8);
                }
            }
            // Re-write bitmap data with updated bits
            for i in 0..bm_sectors {
                let offset = i * SECTOR_SIZE;
                let end = (offset + SECTOR_SIZE).min(bitmap2.len());
                let sector = first_bm_data + i as u32;
                let dst_off = sector as usize * SECTOR_SIZE;
                // Clear and rewrite
                for b in &mut self.data[dst_off..dst_off + SECTOR_SIZE] {
                    *b = 0;
                }
                self.data[dst_off..dst_off + (end - offset)]
                    .copy_from_slice(&bitmap2[offset..end]);
            }

            let mut indirect_data = [0u8; SECTOR_SIZE];
            let count = (bm_sectors - DIRECT_PTR_CNT).min(PTRS_PER_BLOCK);
            for i in 0..count {
                let s = first_bm_data + (DIRECT_PTR_CNT + i) as u32;
                indirect_data[i * 4..i * 4 + 4].copy_from_slice(&s.to_le_bytes());
            }
            self.write_sector_bytes(indirect_sector, &indirect_data);
        }

        // Write free map inode at sector 0
        self.write_sector_bytes(FREE_MAP_SECTOR, &inode.to_bytes());

        eprintln!(
            "  free map: {} bytes bitmap, data at sectors [{}..{})",
            bitmap.len(),
            first_bm_data,
            first_bm_data + bm_sectors as u32,
        );
    }
}

// ---- Main ------------------------------------------------------------------

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: mkdisk <output.img> <size_mb> [file1.elf file2.elf ...]");
        process::exit(1);
    }

    let output = &args[1];
    let size_mb: usize = args[2].parse().unwrap_or_else(|e| {
        eprintln!("Invalid size: {}", e);
        process::exit(1);
    });
    let files = &args[3..];

    eprintln!("Creating {}MB disk image: {}", size_mb, output);
    let mut disk = DiskBuilder::new(size_mb);

    // Write each file and collect (name, inode_sector) pairs
    let mut file_entries: Vec<(String, u32)> = Vec::new();
    for file_path in files {
        let data = fs::read(file_path).unwrap_or_else(|e| {
            eprintln!("Cannot read '{}': {}", file_path, e);
            process::exit(1);
        });

        let name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);

        // Truncate name to NAME_MAX (14 chars) for the directory entry
        let short_name = if name.len() > NAME_MAX {
            &name[..NAME_MAX]
        } else {
            name
        };

        let inode_sector = disk.write_file(short_name, &data);
        file_entries.push((short_name.to_string(), inode_sector));
    }

    // Write root directory
    disk.write_root_dir(&file_entries);

    // Write free map (must be last, after all allocations)
    disk.write_free_map();

    // Write the disk image
    fs::write(output, &disk.data).unwrap_or_else(|e| {
        eprintln!("Cannot write '{}': {}", output, e);
        process::exit(1);
    });

    eprintln!(
        "Done. {} sectors used out of {} ({} bytes)",
        disk.used.iter().filter(|&&u| u).count(),
        disk.sector_count,
        disk.data.len()
    );
}
