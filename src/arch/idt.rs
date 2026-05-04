//! Interrupt Descriptor Table (IDT) and interrupt dispatch.
//!
//! Ported from Pintos interrupt.c. Sets up the 8259A PIC, builds the
//! 256-entry IDT, and provides handler registration and dispatch.

use crate::arch::gdt;
use crate::arch::intr_stubs;
use crate::arch::port::Port;

// ---- PIC ports ----
const PIC0_CTRL: Port = Port::new(0x20);
const PIC0_DATA: Port = Port::new(0x21);
const PIC1_CTRL: Port = Port::new(0xA0);
const PIC1_DATA: Port = Port::new(0xA1);

/// End-of-interrupt command.
const PIC_EOI: u8 = 0x20;

/// Base vector for hardware IRQs (remapped from 0-15).
const IRQ_BASE: u32 = 0x20;

// ---- Interrupt level ----

/// Whether interrupts are enabled or disabled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntrLevel {
    /// Interrupts disabled (IF=0).
    Off,
    /// Interrupts enabled (IF=1).
    On,
}

/// Return the current interrupt level by inspecting EFLAGS.IF.
pub fn intr_get_level() -> IntrLevel {
    let flags: u32;
    unsafe {
        core::arch::asm!(
            "pushfl",
            "popl {flags}",
            flags = out(reg) flags,
            options(att_syntax, nomem)
        );
    }
    if flags & (1 << 9) != 0 {
        IntrLevel::On
    } else {
        IntrLevel::Off
    }
}

/// Disable interrupts and return the previous level.
pub fn intr_disable() -> IntrLevel {
    let old = intr_get_level();
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    old
}

/// Enable interrupts and return the previous level.
#[allow(dead_code)]
pub fn intr_enable() -> IntrLevel {
    let old = intr_get_level();
    unsafe {
        core::arch::asm!("sti", options(nomem, nostack));
    }
    old
}

/// Set the interrupt level and return the previous one.
pub fn intr_set_level(level: IntrLevel) -> IntrLevel {
    match level {
        IntrLevel::Off => intr_disable(),
        IntrLevel::On => intr_enable(),
    }
}

// ---- Interrupt frame ----

/// CPU state pushed onto the stack by the interrupt stubs and the CPU
/// itself. Must match the layout in intr_stubs.S exactly.
#[repr(C)]
#[allow(dead_code)]
pub struct IntrFrame {
    // Pushed by `pushal` in intr_entry (reverse order of pushal)
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp_dummy: u32,
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
    // Segment registers pushed by intr_entry
    pub gs: u16, pub _pad_gs: u16,
    pub fs: u16, pub _pad_fs: u16,
    pub es: u16, pub _pad_es: u16,
    pub ds: u16, pub _pad_ds: u16,
    // Pushed by the stub
    pub vec_no: u32,
    pub error_code: u32,
    pub frame_pointer: u32,
    // Pushed by CPU on interrupt
    pub eip: u32,
    pub cs: u16, pub _pad_cs: u16,
    pub eflags: u32,
    pub esp: u32,
    pub ss: u16, pub _pad_ss: u16,
}

// ---- Handler registration ----

/// Prototype for an interrupt handler function.
pub type IntrHandlerFunc = fn(&mut IntrFrame);

/// Per-vector handler slot.
#[allow(dead_code)]
struct HandlerEntry {
    handler: Option<IntrHandlerFunc>,
    name: &'static str,
    level: IntrLevel,
}

impl HandlerEntry {
    const fn empty() -> Self {
        HandlerEntry {
            handler: None,
            name: "unknown",
            level: IntrLevel::Off,
        }
    }
}

/// Handler table. 256 entries, one per interrupt vector.
///
/// We can't use `Mutex` this early, but all registration happens during
/// single-threaded init with interrupts off, so this is safe.
static mut HANDLERS: [HandlerEntry; 256] = {
    const EMPTY: HandlerEntry = HandlerEntry::empty();
    [EMPTY; 256]
};

/// Register a handler for an internal (CPU exception) interrupt.
#[allow(dead_code)]
pub fn register_int(
    vec_no: u8,
    dpl: u8,
    level: IntrLevel,
    handler: IntrHandlerFunc,
    name: &'static str,
) {
    unsafe {
        // Re-write the IDT gate with the correct DPL.
        let stub_addr = intr_stubs::intr_stubs[vec_no as usize] as usize as u32;
        IDT[vec_no as usize] = IdtGate::new(stub_addr, gdt::SEL_KCSEG, dpl, GateType::Interrupt);

        HANDLERS[vec_no as usize] = HandlerEntry {
            handler: Some(handler),
            name,
            level,
        };
    }
}

/// Register a handler for an external (hardware IRQ) interrupt.
/// The vector number is the IRQ number (0-15), which gets mapped to
/// vectors 0x20-0x2F.
#[allow(dead_code)]
pub fn register_ext(
    irq: u8,
    handler: IntrHandlerFunc,
    name: &'static str,
) {
    let vec_no = IRQ_BASE as u8 + irq;
    unsafe {
        HANDLERS[vec_no as usize] = HandlerEntry {
            handler: Some(handler),
            name,
            level: IntrLevel::Off,
        };
    }
}

/// Register a handler (general purpose).
#[allow(dead_code)]
pub fn register_handler(
    vec_no: u8,
    dpl: u8,
    level: IntrLevel,
    handler: IntrHandlerFunc,
    name: &'static str,
) {
    unsafe {
        let stub_addr = intr_stubs::intr_stubs[vec_no as usize] as usize as u32;
        IDT[vec_no as usize] = IdtGate::new(stub_addr, gdt::SEL_KCSEG, dpl, GateType::Interrupt);

        HANDLERS[vec_no as usize] = HandlerEntry {
            handler: Some(handler),
            name,
            level,
        };
    }
}

// ---- IDT ----

/// Gate types for IDT entries.
#[derive(Clone, Copy)]
#[repr(u8)]
#[allow(dead_code)]
enum GateType {
    Interrupt = 14, // clears IF
    Trap = 15,      // leaves IF
}

/// A single 64-bit IDT gate descriptor.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtGate {
    low: u32,
    high: u32,
}

impl IdtGate {
    const fn null() -> Self {
        IdtGate { low: 0, high: 0 }
    }

    /// Create an IDT gate.
    ///
    /// - `offset`: address of the handler stub
    /// - `selector`: code segment selector
    /// - `dpl`: descriptor privilege level (0 or 3)
    /// - `gate_type`: interrupt or trap gate
    const fn new(offset: u32, selector: u16, dpl: u8, gate_type: GateType) -> Self {
        let low = (offset & 0xFFFF) | ((selector as u32) << 16);
        let high = (offset & 0xFFFF_0000)
            | (1 << 15)                          // P = 1
            | ((dpl as u32 & 3) << 13)
            | ((gate_type as u32 & 0xF) << 8);
        IdtGate { low, high }
    }
}

/// The IDT register structure.
#[repr(C, packed)]
struct IdtRegister {
    limit: u16,
    base: u32,
}

/// The actual IDT. 256 entries.
static mut IDT: [IdtGate; 256] = [IdtGate::null(); 256];

// ---- Yield-on-return support ----

static mut YIELD_ON_RETURN: bool = false;
static mut IN_EXTERNAL_INTR: bool = false;

/// Request a thread yield when the current external interrupt returns.
pub fn intr_yield_on_return() {
    unsafe { YIELD_ON_RETURN = true; }
}

/// True if we are currently handling an external interrupt.
#[allow(dead_code)]
pub fn intr_context() -> bool {
    unsafe { IN_EXTERNAL_INTR }
}

// ---- Dispatch ----

/// Called from assembly (intr_entry) to dispatch an interrupt.
#[no_mangle]
pub extern "C" fn intr_handler(frame: &mut IntrFrame) {
    let vec_no = frame.vec_no as usize;
    let external = vec_no >= IRQ_BASE as usize && vec_no < (IRQ_BASE as usize + 16);

    if external {
        unsafe { IN_EXTERNAL_INTR = true; }
    }

    // Look up the registered handler.
    let entry = unsafe { &HANDLERS[vec_no] };

    if let Some(handler) = entry.handler {
        // Set interrupt level as requested by the handler registration.
        let old_level = intr_set_level(entry.level);
        handler(frame);
        intr_set_level(old_level);
    } else {
        // Unhandled interrupt.
        if external {
            // Spurious IRQ - just acknowledge.
        } else {
            crate::kprintln!(
                "Unhandled interrupt {:#04x} (error_code={:#x}, eip={:#x})",
                vec_no,
                frame.error_code,
                frame.eip
            );
        }
    }

    // Send EOI to PIC for external interrupts.
    if external {
        // If IRQ came from slave PIC (IRQ 8-15), send EOI to slave first.
        if vec_no >= (IRQ_BASE as usize + 8) {
            PIC1_CTRL.write_u8(PIC_EOI);
        }
        PIC0_CTRL.write_u8(PIC_EOI);

        unsafe {
            IN_EXTERNAL_INTR = false;
            if YIELD_ON_RETURN {
                YIELD_ON_RETURN = false;
                crate::thread::yield_current();
            }
        }
    }
}

// ---- Initialization ----

/// Initialize the PIC (8259A), IDT, and load it.
pub fn init() {
    // 1. Initialize the 8259A PICs.
    pic_init();

    // 2. Build the IDT from the stub table.
    unsafe {
        let idt = (&raw mut IDT).as_mut().unwrap();
        for (i, entry) in idt.iter_mut().enumerate() {
            let stub_addr = intr_stubs::intr_stubs[i] as usize as u32;
            *entry = IdtGate::new(stub_addr, gdt::SEL_KCSEG, 0, GateType::Interrupt);
        }

        // Load the IDT register.
        let idtr = IdtRegister {
            limit: (core::mem::size_of::<[IdtGate; 256]>() - 1) as u16,
            base: (&raw const IDT) as *const _ as u32,
        };

        core::arch::asm!(
            "lidt ({idtr})",
            idtr = in(reg) &idtr,
            options(att_syntax, nostack)
        );
    }

    crate::kprintln!("IDT initialized ({} entries).", 256);
}

/// Initialize the 8259A Programmable Interrupt Controllers.
///
/// Remaps IRQ 0-7 to vectors 0x20-0x27 and IRQ 8-15 to vectors 0x28-0x2F.
/// All IRQs are initially masked (disabled).
fn pic_init() {
    // ICW1: start initialization sequence, expect ICW4.
    PIC0_CTRL.write_u8(0x11);
    PIC1_CTRL.write_u8(0x11);

    // ICW2: base interrupt vector.
    PIC0_DATA.write_u8(IRQ_BASE as u8);         // Master: vectors 0x20-0x27
    PIC1_DATA.write_u8((IRQ_BASE + 8) as u8);   // Slave:  vectors 0x28-0x2F

    // ICW3: cascade wiring.
    PIC0_DATA.write_u8(0x04);   // Master: slave on IRQ2
    PIC1_DATA.write_u8(0x02);   // Slave:  cascade identity = 2

    // ICW4: 8086 mode.
    PIC0_DATA.write_u8(0x01);
    PIC1_DATA.write_u8(0x01);

    // Mask all IRQs (will be unmasked individually when handlers register).
    PIC0_DATA.write_u8(0xFF);
    PIC1_DATA.write_u8(0xFF);
}

/// Unmask an IRQ line on the PIC so it can deliver interrupts.
#[allow(dead_code)]
pub fn pic_unmask(irq: u8) {
    if irq < 8 {
        let mask = PIC0_DATA.read_u8() & !(1 << irq);
        PIC0_DATA.write_u8(mask);
    } else {
        let mask = PIC1_DATA.read_u8() & !(1 << (irq - 8));
        PIC1_DATA.write_u8(mask);
        // Also unmask IRQ2 on master (cascade line).
        let master_mask = PIC0_DATA.read_u8() & !(1 << 2);
        PIC0_DATA.write_u8(master_mask);
    }
}
