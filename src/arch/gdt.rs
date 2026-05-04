//! Global Descriptor Table (GDT) setup.
//!
//! Ported from Pintos gdt.c. Sets up flat-model segmentation with
//! separate kernel code and data segments.

/// Kernel code segment selector.
pub const SEL_KCSEG: u16 = 0x08;
/// Kernel data segment selector.
pub const SEL_KDSEG: u16 = 0x10;
/// User code segment selector (ring 3).
#[allow(dead_code)]
pub const SEL_UCSEG: u16 = 0x1B;
/// User data segment selector (ring 3).
#[allow(dead_code)]
pub const SEL_UDSEG: u16 = 0x23;
/// TSS segment selector.
#[allow(dead_code)]
pub const SEL_TSS: u16 = 0x28;

/// Number of GDT entries.
const GDT_COUNT: usize = 6;

/// A single 64-bit segment descriptor, stored as two 32-bit halves.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct SegmentDescriptor {
    low: u32,
    high: u32,
}

impl SegmentDescriptor {
    /// Create a null descriptor.
    const fn null() -> Self {
        SegmentDescriptor { low: 0, high: 0 }
    }

    /// Create a segment descriptor with the given parameters.
    ///
    /// - `base`: 32-bit base address
    /// - `limit`: 20-bit limit (in units determined by granularity)
    /// - `seg_type`: 4-bit type field
    /// - `s`: 1 = code/data, 0 = system
    /// - `dpl`: descriptor privilege level (0-3)
    /// - `present`: present bit
    /// - `db`: default operation size (1 = 32-bit)
    /// - `granularity`: 1 = 4KB granularity, 0 = byte granularity
    #[allow(clippy::too_many_arguments)]
    const fn new(
        base: u32,
        limit: u32,
        seg_type: u8,
        s: u8,
        dpl: u8,
        present: u8,
        db: u8,
        granularity: u8,
    ) -> Self {
        let low = (limit & 0xFFFF) | ((base & 0xFFFF) << 16);

        let high = ((base >> 16) & 0xFF)
            | ((seg_type as u32 & 0xF) << 8)
            | ((s as u32 & 1) << 12)
            | ((dpl as u32 & 3) << 13)
            | ((present as u32 & 1) << 15)
            | ((limit >> 16) & 0xF) << 16
            | ((db as u32 & 1) << 22)
            | ((granularity as u32 & 1) << 23)
            | ((base >> 24) & 0xFF) << 24;

        SegmentDescriptor { low, high }
    }

    /// Flat kernel code segment: base=0, limit=4GB, DPL=0, execute/read.
    const fn kernel_code() -> Self {
        Self::new(
            0,          // base
            0xFFFFF,    // limit (4GB with page granularity)
            0xA,        // type: execute/read
            1,          // S: code/data
            0,          // DPL: ring 0
            1,          // present
            1,          // D/B: 32-bit
            1,          // granularity: 4KB
        )
    }

    /// Flat kernel data segment: base=0, limit=4GB, DPL=0, read/write.
    const fn kernel_data() -> Self {
        Self::new(
            0,          // base
            0xFFFFF,    // limit
            0x2,        // type: read/write
            1,          // S: code/data
            0,          // DPL: ring 0
            1,          // present
            1,          // D/B: 32-bit
            1,          // granularity: 4KB
        )
    }

    /// Flat user code segment: base=0, limit=4GB, DPL=3, execute/read.
    const fn user_code() -> Self {
        Self::new(0, 0xFFFFF, 0xA, 1, 3, 1, 1, 1)
    }

    /// Flat user data segment: base=0, limit=4GB, DPL=3, read/write.
    const fn user_data() -> Self {
        Self::new(0, 0xFFFFF, 0x2, 1, 3, 1, 1, 1)
    }
}

/// The GDTR register value: 6-byte packed structure.
#[repr(C, packed)]
struct GdtRegister {
    limit: u16,
    base: u32,
}

/// The actual GDT table. Statically allocated.
static mut GDT: [SegmentDescriptor; GDT_COUNT] = [SegmentDescriptor::null(); GDT_COUNT];

/// Update the TSS descriptor in the GDT (slot 5, selector 0x28).
///
/// Sets up a system segment descriptor with:
/// type=0x9 (available 32-bit TSS), S=0 (system), DPL=0, P=1, G=0 (byte granularity).
pub fn set_tss(base: u32, limit: u16) {
    unsafe {
        let low = (limit as u32 & 0xFFFF) | ((base & 0xFFFF) << 16);
        #[allow(clippy::identity_op)]
        let high = ((base >> 16) & 0xFF)
            | (0x9 << 8)   // type: available 32-bit TSS
            | (0 << 12)    // S: system segment
            | (0 << 13)    // DPL: 0
            | (1 << 15)    // P: present
            | (((limit as u32 >> 16) & 0xF) << 16)
            | (0 << 22)    // D/B: 0 for TSS
            | (0 << 23)    // G: byte granularity
            | (((base >> 24) & 0xFF) << 24);
        GDT[5] = SegmentDescriptor { low, high };
    }
}

/// Initialize the GDT and load it via `lgdt`.
///
/// Sets up kernel code and data segments. User segments and TSS are
/// placeholder entries for now (will be populated in later phases).
pub fn init() {
    unsafe {
        GDT[0] = SegmentDescriptor::null();         // 0x00: NULL
        GDT[1] = SegmentDescriptor::kernel_code();  // 0x08: KCSEG
        GDT[2] = SegmentDescriptor::kernel_data();  // 0x10: KDSEG
        GDT[3] = SegmentDescriptor::user_code();    // 0x18: UCSEG (+ RPL 3 = 0x1B)
        GDT[4] = SegmentDescriptor::user_data();    // 0x20: UDSEG (+ RPL 3 = 0x23)
        GDT[5] = SegmentDescriptor::null();          // 0x28: TSS (set up later)

        let gdtr = GdtRegister {
            limit: (core::mem::size_of::<[SegmentDescriptor; GDT_COUNT]>() - 1) as u16,
            base: (&raw const GDT) as *const _ as u32,
        };

        // Load the GDT register.
        core::arch::asm!(
            "lgdt ({gdtr})",
            gdtr = in(reg) &gdtr,
            options(att_syntax, nostack)
        );

        // Reload segment registers with new selectors.
        // Far jump to reload CS with KCSEG.
        core::arch::asm!(
            "ljmp $0x08, $2f",
            "2:",
            // Reload data segment registers with KDSEG.
            "mov $0x10, %ax",
            "mov %ax, %ds",
            "mov %ax, %es",
            "mov %ax, %fs",
            "mov %ax, %gs",
            "mov %ax, %ss",
            options(att_syntax, nostack)
        );
    }

    crate::kprintln!("GDT initialized.");
}
