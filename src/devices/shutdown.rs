//! Machine shutdown and reboot.
//!
//! Ported from Pintos shutdown.c. Supports QEMU and Bochs.

use crate::arch::port::Port;

/// Power off the machine (works in QEMU/Bochs).
#[allow(dead_code)]
pub fn power_off() -> ! {
    crate::kprintln!("Powering off...");

    // ACPI shutdown (Bochs, older QEMU)
    Port::new(0xB004).write_u16(0x2000);

    // QEMU debug exit (isa-debug-exit device)
    for &c in b"Shutdown" {
        Port::new(0x8900).write_u8(c);
    }

    // Newer QEMU (isa-debug-exit at 0x501)
    Port::new(0x0501).write_u8(0x31);

    loop {
        unsafe {
            core::arch::asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

/// Reboot via the 8042 keyboard controller reset command.
#[allow(dead_code)]
pub fn reboot() -> ! {
    crate::kprintln!("Rebooting...");
    let ctrl = Port::new(0x64);
    loop {
        // Wait for the controller input buffer to clear.
        while ctrl.read_u8() & 0x02 != 0 {
            core::hint::spin_loop();
        }
        // Send system reset command.
        ctrl.write_u8(0xFE);
    }
}
