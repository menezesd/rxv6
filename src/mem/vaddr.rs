//! Virtual/physical address types and page-table entry constants.
//!
//! Ported from Pintos vaddr.h and pte.h.

/// Page size in bytes (4 KB).
pub const PGSIZE: usize = 4096;

/// Maximum user stack size (8 MB).
pub const USER_STACK_MAX: usize = 8 * 1024 * 1024;

/// Number of offset bits in a virtual address.
pub const PGBITS: usize = 12;

/// Bitmask for the page offset.
pub const PGMASK: usize = 0xFFF;

/// Kernel virtual base address. Physical address 0 maps here.
pub const PHYS_BASE: usize = 0xC000_0000;

// ── Address conversion ─────────────────────────────────────────────

/// Convert a physical address to a kernel virtual address.
#[inline]
pub fn ptov(paddr: usize) -> usize {
    debug_assert!(paddr < PHYS_BASE, "ptov: physical address too large");
    paddr + PHYS_BASE
}

/// Convert a kernel virtual address to a physical address.
#[inline]
pub fn vtop(vaddr: usize) -> usize {
    debug_assert!(is_kernel_vaddr(vaddr), "vtop: not a kernel virtual address");
    vaddr - PHYS_BASE
}

/// Round `addr` up to the next page boundary.
#[inline]
pub fn pg_round_up(addr: usize) -> usize {
    (addr + PGSIZE - 1) & !PGMASK
}

/// Round `addr` down to a page boundary.
#[inline]
pub fn pg_round_down(addr: usize) -> usize {
    addr & !PGMASK
}

/// Return the offset within the page.
#[inline]
#[allow(dead_code)]
pub fn pg_ofs(addr: usize) -> usize {
    addr & PGMASK
}

/// True if `addr` is in the kernel half of the address space.
#[inline]
pub fn is_kernel_vaddr(addr: usize) -> bool {
    addr >= PHYS_BASE
}

// ── Page-table / page-directory entry flags ─────────────────────────

/// Present bit.
pub const PTE_P: u32 = 0x1;
/// Writable bit.
pub const PTE_W: u32 = 0x2;
/// User-accessible bit.
pub const PTE_U: u32 = 0x4;
/// Accessed bit (set by CPU on read).
#[allow(dead_code)]
pub const PTE_A: u32 = 0x20;
/// Dirty bit (set by CPU on write).
#[allow(dead_code)]
pub const PTE_D: u32 = 0x40;
/// Mask for the physical address portion of a PTE.
#[allow(dead_code)]
pub const PTE_ADDR: u32 = 0xFFFF_F000;
