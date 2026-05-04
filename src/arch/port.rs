//! Safe wrapper around x86 I/O port operations.
//!
//! All `unsafe` port I/O is contained here. Higher-level code uses
//! `Port` values and safe read/write methods.

/// A typed I/O port at a fixed address.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Port {
    addr: u16,
}

#[allow(dead_code)]
impl Port {
    /// Create a port handle for the given I/O address.
    pub const fn new(addr: u16) -> Self {
        Port { addr }
    }

    /// Read a byte from this port.
    pub fn read_u8(&self) -> u8 {
        unsafe {
            let val: u8;
            core::arch::asm!(
                "in al, dx",
                out("al") val,
                in("dx") self.addr,
                options(nomem, nostack, preserves_flags)
            );
            val
        }
    }

    /// Write a byte to this port.
    pub fn write_u8(&self, val: u8) {
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("al") val,
                in("dx") self.addr,
                options(nomem, nostack, preserves_flags)
            );
        }
    }

    /// Read a 16-bit word from this port.
    pub fn read_u16(&self) -> u16 {
        unsafe {
            let val: u16;
            core::arch::asm!(
                "in ax, dx",
                out("ax") val,
                in("dx") self.addr,
                options(nomem, nostack, preserves_flags)
            );
            val
        }
    }

    /// Write a 16-bit word to this port.
    pub fn write_u16(&self, val: u16) {
        unsafe {
            core::arch::asm!(
                "out dx, ax",
                in("ax") val,
                in("dx") self.addr,
                options(nomem, nostack, preserves_flags)
            );
        }
    }

    /// Read a 32-bit dword from this port.
    pub fn read_u32(&self) -> u32 {
        unsafe {
            let val: u32;
            core::arch::asm!(
                "in eax, dx",
                out("eax") val,
                in("dx") self.addr,
                options(nomem, nostack, preserves_flags)
            );
            val
        }
    }

    /// Write a 32-bit dword to this port.
    pub fn write_u32(&self, val: u32) {
        unsafe {
            core::arch::asm!(
                "out dx, eax",
                in("eax") val,
                in("dx") self.addr,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}
