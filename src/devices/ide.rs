//! IDE/ATA disk driver.
//!
//! Ported from Pintos devices/ide.c. Supports two channels (primary and
//! secondary) with up to two devices each (master and slave).

use alloc::boxed::Box;
use crate::arch::idt::{self, IntrFrame};
use crate::arch::port::Port;
use crate::sync::{Lock, Semaphore};
use crate::devices::block::{self, BlockSector, BlockOps, BLOCK_SECTOR_SIZE};

// ---- ATA register offsets from channel base ----
const REG_DATA: u16 = 0;       // 16-bit data
#[allow(dead_code)]
const REG_ERROR: u16 = 1;      // error (read)
const REG_NSECT: u16 = 2;      // sector count
const REG_LBAL: u16 = 3;       // LBA low  [7:0]
const REG_LBAM: u16 = 4;       // LBA mid  [15:8]
const REG_LBAH: u16 = 5;       // LBA high [23:16]
const REG_DEVICE: u16 = 6;     // device/head
const REG_STATUS: u16 = 7;     // status (read) / command (write)
const REG_COMMAND: u16 = 7;

// ---- Status register bits ----
const STA_BSY: u8 = 0x80;
#[allow(dead_code)]
const STA_DRDY: u8 = 0x40;
const STA_DRQ: u8 = 0x08;

// ---- Commands ----
const CMD_IDENTIFY: u8 = 0xEC;
const CMD_READ: u8 = 0x20;
const CMD_WRITE: u8 = 0x30;

// ---- Device register bits ----
const DEV_MBS: u8 = 0xA0;      // must-be-set bits
const DEV_LBA: u8 = 0x40;      // LBA mode
const DEV_DEV1: u8 = 0x10;     // select slave

// ---- Channel configuration ----
const CHANNEL_CNT: usize = 2;

#[allow(dead_code)]
struct Channel {
    name: [u8; 8],
    reg_base: u16,
    irq: u8,
    lock: Lock,
    expecting_interrupt: bool,
    completion: Semaphore,
    devices: [AtaDisk; 2],
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct AtaDisk {
    name: [u8; 8],
    channel_idx: usize,
    dev_no: u8,         // 0 = master, 1 = slave
    is_ata: bool,
    size: BlockSector,  // size in sectors
}

impl AtaDisk {
    #[allow(dead_code)]
    const fn empty() -> Self {
        AtaDisk {
            name: [0u8; 8],
            channel_idx: 0,
            dev_no: 0,
            is_ata: false,
            size: 0,
        }
    }
}

/// Static storage for the two IDE channels.
static mut CHANNELS: Option<alloc::vec::Vec<Channel>> = None;

/// IDE interrupt handler for channel 0 (IRQ 14).
fn ide_interrupt_ch0(_frame: &mut IntrFrame) {
    ide_interrupt(0);
}

/// IDE interrupt handler for channel 1 (IRQ 15).
fn ide_interrupt_ch1(_frame: &mut IntrFrame) {
    ide_interrupt(1);
}

fn ide_interrupt(ch_idx: usize) {
    unsafe {
        if let Some(ref mut channels) = CHANNELS {
            let ch = &mut channels[ch_idx];
            if ch.expecting_interrupt {
                // Read status to acknowledge the interrupt.
                Port::new(ch.reg_base + REG_STATUS).read_u8();
                ch.expecting_interrupt = false;
                ch.completion.up();
            }
        }
    }
}

/// Wait until the channel is not busy (BSY clear).
fn wait_not_busy(base: u16) {
    let status_port = Port::new(base + REG_STATUS);
    // Busy-wait with a ~1s timeout.
    for _ in 0..100_000u32 {
        let st = status_port.read_u8();
        if st == 0xFF {
            return; // no device on bus (floating bus reads 0xFF)
        }
        if st & STA_BSY == 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Wait until the device is ready (BSY clear and DRDY set), or return false.
#[allow(dead_code)]
fn wait_drdy(base: u16) -> bool {
    let status_port = Port::new(base + REG_STATUS);
    for _ in 0..300_000u32 {
        let s = status_port.read_u8();
        if s & STA_BSY == 0 && s & STA_DRDY != 0 {
            return true;
        }
        for _ in 0..1000u32 {
            core::hint::spin_loop();
        }
    }
    false
}

/// Busy-wait for approximately `ms` milliseconds using a spin loop.
/// This is a rough approximation used only during IDE initialization
/// before we have a calibrated timer delay.
fn busy_delay_ms(ms: u32) {
    // ~150k iterations per ms on typical QEMU -- conservative estimate.
    for _ in 0..ms {
        for _ in 0..150_000u32 {
            core::hint::spin_loop();
        }
    }
}

/// Read 256 16-bit words from the data port into `buf`.
fn insw(base: u16, buf: &mut [u8; BLOCK_SECTOR_SIZE]) {
    let port = Port::new(base + REG_DATA);
    let ptr = buf.as_mut_ptr() as *mut u16;
    for i in 0..256 {
        unsafe { *ptr.add(i) = port.read_u16(); }
    }
}

/// Write 256 16-bit words from `buf` to the data port.
fn outsw(base: u16, buf: &[u8; BLOCK_SECTOR_SIZE]) {
    let port = Port::new(base + REG_DATA);
    let ptr = buf.as_ptr() as *const u16;
    for i in 0..256 {
        unsafe { port.write_u16(*ptr.add(i)); }
    }
}

/// Select a sector for the next read/write on the given channel.
fn select_sector(base: u16, dev_no: u8, sector: BlockSector) {
    Port::new(base + REG_NSECT).write_u8(1);
    Port::new(base + REG_LBAL).write_u8((sector & 0xFF) as u8);
    Port::new(base + REG_LBAM).write_u8(((sector >> 8) & 0xFF) as u8);
    Port::new(base + REG_LBAH).write_u8(((sector >> 16) & 0xFF) as u8);
    let dev_bits = DEV_MBS | DEV_LBA
        | if dev_no == 1 { DEV_DEV1 } else { 0 }
        | ((sector >> 24) & 0x0F) as u8;
    Port::new(base + REG_DEVICE).write_u8(dev_bits);
}

/// Select a device (master/slave) on the given channel base.
fn select_device(base: u16, dev_no: u8) {
    let dev_bits = DEV_MBS | DEV_LBA | if dev_no == 1 { DEV_DEV1 } else { 0 };
    Port::new(base + REG_DEVICE).write_u8(dev_bits);
    // Wait a bit after device selection (400ns = ~4 status reads).
    let status_port = Port::new(base + REG_STATUS);
    for _ in 0..4 {
        status_port.read_u8();
    }
}

/// Issue a command and set up the channel to expect an interrupt.
fn issue_command(ch_idx: usize, cmd: u8) {
    unsafe {
        if let Some(ref mut channels) = CHANNELS {
            let ch = &mut channels[ch_idx];
            ch.expecting_interrupt = true;
            Port::new(ch.reg_base + REG_COMMAND).write_u8(cmd);
        }
    }
}

/// Wait for the channel's completion semaphore (interrupt-driven).
fn wait_completion(ch_idx: usize) -> bool {
    // Wait for interrupt with a timeout. If no device is present, the
    // interrupt never fires, so we poll the semaphore with yields.
    let deadline = crate::devices::timer::ticks() + 200; // 2 second timeout
    unsafe {
        if let Some(ref mut channels) = CHANNELS {
            let ch = &mut channels[ch_idx];
            loop {
                if ch.completion.try_down() {
                    return true;
                }
                if crate::devices::timer::ticks() >= deadline {
                    ch.expecting_interrupt = false;
                    return false; // timeout
                }
                crate::thread::yield_current();
            }
        }
    }
    false
}

/// Check status after a command. Returns true if no error.
fn check_status(base: u16) -> bool {
    let s = Port::new(base + REG_STATUS).read_u8();
    // Check for error bit (bit 0) or device fault (bit 5).
    s & 0x21 == 0
}

// ---- BlockOps implementation for an ATA disk ----

/// Wraps channel index and device number so we can implement BlockOps.
struct IdeBlockOps {
    channel_idx: usize,
    dev_no: u8,
}

// Safety: we synchronize access through channel locks.
unsafe impl Send for IdeBlockOps {}
unsafe impl Sync for IdeBlockOps {}

impl BlockOps for IdeBlockOps {
    fn read(&self, sector: BlockSector, buf: &mut [u8; BLOCK_SECTOR_SIZE]) {
        unsafe {
            if let Some(ref mut channels) = CHANNELS {
                let base = channels[self.channel_idx].reg_base;
                channels[self.channel_idx].lock.acquire();

                select_sector(base, self.dev_no, sector);
                issue_command(self.channel_idx, CMD_READ);
                wait_completion(self.channel_idx);

                if !check_status(base) {
                    crate::kprintln!("ide: read error on sector {}", sector);
                }

                insw(base, buf);
                channels[self.channel_idx].lock.release();
            }
        }
    }

    fn write(&self, sector: BlockSector, buf: &[u8; BLOCK_SECTOR_SIZE]) {
        unsafe {
            if let Some(ref mut channels) = CHANNELS {
                let base = channels[self.channel_idx].reg_base;
                channels[self.channel_idx].lock.acquire();

                select_sector(base, self.dev_no, sector);
                issue_command(self.channel_idx, CMD_WRITE);

                // For writes: wait for DRQ (device ready for data), THEN send data.
                if !check_status(base) {
                    crate::kprintln!("ide: write error on sector {}", sector);
                    channels[self.channel_idx].lock.release();
                    return;
                }
                // Wait for DRQ.
                let status_port = Port::new(base + REG_STATUS);
                for _ in 0..100_000u32 {
                    if status_port.read_u8() & STA_DRQ != 0 {
                        break;
                    }
                    core::hint::spin_loop();
                }

                outsw(base, buf);

                // Now wait for completion interrupt.
                wait_completion(self.channel_idx);
                channels[self.channel_idx].lock.release();
            }
        }
    }
}

/// Device names: hda, hdb, hdc, hdd.
const DEV_NAMES: [[u8; 8]; 4] = [
    [b'h', b'd', b'a', 0, 0, 0, 0, 0],
    [b'h', b'd', b'b', 0, 0, 0, 0, 0],
    [b'h', b'd', b'c', 0, 0, 0, 0, 0],
    [b'h', b'd', b'd', 0, 0, 0, 0, 0],
];

const CHANNEL_NAMES: [[u8; 8]; 2] = [
    [b'i', b'd', b'e', b'0', 0, 0, 0, 0],
    [b'i', b'd', b'e', b'1', 0, 0, 0, 0],
];

/// Initialize the IDE subsystem. Probes both channels and registers
/// any disks found with the block device layer.
pub fn init() {
    let channel_configs: [(u16, u16, u8); 2] = [
        (0x1F0, 0x3F6, 14), // primary
        (0x170, 0x376, 15), // secondary
    ];

    // Build channels on the heap one at a time to avoid stack overflow.
    // Each Channel contains Lock + Semaphore (with VecDeque), too large for 4KB stack.
    let mut channels = alloc::vec::Vec::with_capacity(CHANNEL_CNT);
    for i in 0..CHANNEL_CNT {
        let (base, _ctrl, irq) = channel_configs[i];
        channels.push(Channel {
            name: CHANNEL_NAMES[i],
            reg_base: base,
            irq,
            lock: Lock::new(),
            expecting_interrupt: false,
            completion: Semaphore::new(0),
            devices: [
                AtaDisk { name: DEV_NAMES[i * 2], channel_idx: i, dev_no: 0, is_ata: false, size: 0 },
                AtaDisk { name: DEV_NAMES[i * 2 + 1], channel_idx: i, dev_no: 1, is_ata: false, size: 0 },
            ],
        });
    }

    unsafe { CHANNELS = Some(channels); }

    // Register interrupt handlers and unmask IRQs.
    idt::register_ext(14, ide_interrupt_ch0, "ide0");
    idt::register_ext(15, ide_interrupt_ch1, "ide1");
    idt::pic_unmask(14);
    idt::pic_unmask(15);

    // Probe each channel.
    for ch_idx in 0..CHANNEL_CNT {
        let base = channel_configs[ch_idx].0;

        // Software reset the channel.
        // Write SRST bit (0x04) + nIEN bit (0x02) to control register.
        let ctrl_port = Port::new(channel_configs[ch_idx].1);
        ctrl_port.write_u8(0x04 | 0x02);  // assert SRST
        busy_delay_ms(10);
        ctrl_port.write_u8(0x02);          // de-assert SRST, keep nIEN set
        busy_delay_ms(150);

        // Re-enable interrupts on this channel.
        ctrl_port.write_u8(0x00);

        wait_not_busy(base);

        // Probe each device on this channel.
        for dev_no in 0u8..2 {
            select_device(base, dev_no);

            // Write a test pattern to sector count and LBA low.
            Port::new(base + REG_NSECT).write_u8(0x55);
            Port::new(base + REG_LBAL).write_u8(0xAA);

            // Read them back.
            let nsect = Port::new(base + REG_NSECT).read_u8();
            let lbal = Port::new(base + REG_LBAL).read_u8();

            if nsect != 0x55 || lbal != 0xAA {
                // No device present.
                continue;
            }

            // Device present. Send IDENTIFY DEVICE.
            unsafe {
                if let Some(ref mut channels) = CHANNELS {
                    channels[ch_idx].expecting_interrupt = true;
                }
            }
            Port::new(base + REG_COMMAND).write_u8(CMD_IDENTIFY);

            // Wait for completion interrupt (with timeout).
            if !wait_completion(ch_idx) {
                // Timeout - no device or device not responding.
                continue;
            }

            // Check status.
            if !check_status(base) {
                // Not an ATA device (could be ATAPI or nothing).
                continue;
            }

            // Wait for DRQ.
            let status_port = Port::new(base + REG_STATUS);
            let mut have_drq = false;
            for _ in 0..100_000u32 {
                if status_port.read_u8() & STA_DRQ != 0 {
                    have_drq = true;
                    break;
                }
                core::hint::spin_loop();
            }
            if !have_drq {
                continue;
            }

            // Read IDENTIFY data (256 words = 512 bytes).
            // Heap-allocate to avoid stack overflow on the 4KB thread stack.
            let mut id_buf = alloc::boxed::Box::new([0u8; BLOCK_SECTOR_SIZE]);
            insw(base, &mut id_buf);

            // Disk size is at word offsets 60-61 (little-endian u32).
            // Word 60 is at byte offset 120, word 61 at byte offset 122.
            let size_lo = u16::from_le_bytes([id_buf[120], id_buf[121]]) as u32;
            let size_hi = u16::from_le_bytes([id_buf[122], id_buf[123]]) as u32;
            let size = (size_hi << 16) | size_lo;

            if size == 0 {
                continue;
            }

            // Mark as ATA device.
            unsafe {
                if let Some(ref mut channels) = CHANNELS {
                    let disk = &mut channels[ch_idx].devices[dev_no as usize];
                    disk.is_ata = true;
                    disk.size = size;

                    let name_str = dev_name_str(ch_idx, dev_no as usize);
                    let size_kb = (size as u64 * BLOCK_SECTOR_SIZE as u64) / 1024;
                    crate::kprintln!(
                        "ide: {} is {} KB ({} sectors)",
                        name_str,
                        size_kb,
                        size
                    );

                    // Register with block device layer.
                    let ops = Box::new(IdeBlockOps {
                        channel_idx: ch_idx,
                        dev_no,
                    });
                    block::register(name_str, block::BlockType::Raw, size, ops);
                }
            }
        }
    }
}

/// Get a device name string from channel/device indices.
fn dev_name_str(ch_idx: usize, dev_idx: usize) -> &'static str {
    match (ch_idx, dev_idx) {
        (0, 0) => "hda",
        (0, 1) => "hdb",
        (1, 0) => "hdc",
        (1, 1) => "hdd",
        _ => "hd?",
    }
}
