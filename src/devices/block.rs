//! Block device trait and global registry.
//!
//! Ported from Pintos devices/block.c / block.h.

use alloc::boxed::Box;
use alloc::vec::Vec;

pub const BLOCK_SECTOR_SIZE: usize = 512;
pub type BlockSector = u32;

/// Heap-allocate a zeroed sector buffer (avoids 512-byte stack temp on 4KB thread stacks).
pub fn sector_buf() -> Box<[u8; BLOCK_SECTOR_SIZE]> {
    let layout = core::alloc::Layout::new::<[u8; BLOCK_SECTOR_SIZE]>();
    // Safety: layout is non-zero (512 bytes). We own the returned pointer
    // exclusively and wrap it in Box for automatic deallocation.
    unsafe {
        let ptr = alloc::alloc::alloc_zeroed(layout) as *mut [u8; BLOCK_SECTOR_SIZE];
        assert!(!ptr.is_null(), "sector_buf: heap allocation failed");
        Box::from_raw(ptr)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum BlockType {
    Kernel,
    FileSys,
    Scratch,
    Swap,
    Raw,
    Foreign,
}

/// Block device trait -- must be implemented by drivers (IDE, etc.).
pub trait BlockOps: Send {
    fn read(&self, sector: BlockSector, buf: &mut [u8; BLOCK_SECTOR_SIZE]);
    fn write(&self, sector: BlockSector, buf: &[u8; BLOCK_SECTOR_SIZE]);
}

/// A registered block device.
#[allow(dead_code)]
pub struct BlockDevice {
    pub name: [u8; 16],
    pub block_type: BlockType,
    pub size: BlockSector,
    ops: Box<dyn BlockOps>,
    read_cnt: u64,
    write_cnt: u64,
}

#[allow(dead_code)]
impl BlockDevice {
    pub fn read(&mut self, sector: BlockSector, buf: &mut [u8; BLOCK_SECTOR_SIZE]) {
        assert!(sector < self.size, "sector {} out of bounds (size {})", sector, self.size);
        self.ops.read(sector, buf);
        self.read_cnt += 1;
    }

    pub fn write(&mut self, sector: BlockSector, buf: &[u8; BLOCK_SECTOR_SIZE]) {
        assert!(sector < self.size, "sector {} out of bounds (size {})", sector, self.size);
        self.ops.write(sector, buf);
        self.write_cnt += 1;
    }

    pub fn name_str(&self) -> &str {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..len]).unwrap_or("???")
    }
}

/// Global block device registry.
static mut DEVICES: Vec<BlockDevice> = Vec::new();

/// Role-based device lookup indices (FileSys=0, Scratch=1, Swap=2, Raw=3).
static mut ROLES: [Option<usize>; 4] = [None; 4];

fn role_index(role: BlockType) -> Option<usize> {
    match role {
        BlockType::FileSys => Some(0),
        BlockType::Scratch => Some(1),
        BlockType::Swap => Some(2),
        BlockType::Raw => Some(3),
        _ => None,
    }
}

/// Register a new block device. Returns its index in the registry.
#[allow(dead_code)]
pub fn register(
    name: &str,
    block_type: BlockType,
    size: BlockSector,
    ops: Box<dyn BlockOps>,
) -> usize {
    let mut name_buf = [0u8; 16];
    let len = name.len().min(15);
    name_buf[..len].copy_from_slice(&name.as_bytes()[..len]);

    let dev = BlockDevice {
        name: name_buf,
        block_type,
        size,
        ops,
        read_cnt: 0,
        write_cnt: 0,
    };

    let devs = unsafe { (&raw mut DEVICES).as_mut().unwrap() };
    let idx = devs.len();
    crate::kprintln!("block: registered \"{}\" ({:?}, {} sectors)", name, block_type, size);
    devs.push(dev);
    idx
}

/// Look up a device by name.
#[allow(dead_code)]
pub fn get_by_name(name: &str) -> Option<&'static mut BlockDevice> {
    let devs = unsafe { (&raw mut DEVICES).as_mut().unwrap() };
    devs.iter_mut().find(|d| d.name_str() == name)
}

/// Get the device assigned to a particular role.
#[allow(dead_code)]
pub fn get_role(role: BlockType) -> Option<&'static mut BlockDevice> {
    let ri = role_index(role)?;
    let roles = unsafe { (&raw const ROLES).as_ref().unwrap() };
    let idx = roles[ri]?;
    let devs = unsafe { (&raw mut DEVICES).as_mut().unwrap() };
    devs.get_mut(idx)
}

/// Assign a role to a device by its registry index.
#[allow(dead_code)]
pub fn set_role(role: BlockType, dev_idx: usize) {
    if let Some(ri) = role_index(role) {
        unsafe {
            ROLES[ri] = Some(dev_idx);
        }
    }
}
