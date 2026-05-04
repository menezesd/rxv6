//! rxv6 -- a Rust reimplementation of UNIX v6 for x86-32.
//!
//! Based on xv6 (MIT) and infrastructure from the RustOS/PintOS port.
//! Targets i686 bare metal, boots via Multiboot, runs under QEMU.

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

/// Access a `static mut Option<T>` via raw pointer, returning `&'static mut T`.
#[macro_export]
macro_rules! static_mut {
    ($name:ident) => {
        unsafe {
            (&raw mut $name).as_mut().unwrap()
                .as_mut().expect(concat!(stringify!($name), " not initialized"))
        }
    };
}

mod arch;
mod console;
mod devices;
mod filesys;
mod mem;
mod sync;
mod thread;
mod userprog;
mod vm;

use core::panic::PanicInfo;

core::arch::global_asm!(include_str!("../asm/start.S"), options(att_syntax));

/// Kernel entry point.
#[no_mangle]
pub extern "C" fn main() -> ! {
    // Console
    devices::serial::init();
    devices::vga::clear();
    kprintln!("rxv6 kernel booting...");

    // CPU tables + memory
    arch::gdt::init();
    arch::idt::init();
    mem::palloc::init(0);
    mem::malloc::init();
    mem::paging::init();

    // Threading + user program support
    thread::init();
    userprog::tss::init();
    userprog::process::init();
    userprog::exception::init();
    userprog::syscall::init();
    devices::timer::init();
    thread::start();
    devices::timer::calibrate();

    // Devices
    devices::keyboard::init();
    devices::ide::init();

    // Virtual memory
    vm::frame::init();
    vm::swap::init();

    kprintln!("cpu, mem, vm, devices: ok");

    // Filesystem
    if devices::block::get_by_name("hda").is_some() {
        devices::block::set_role(devices::block::BlockType::FileSys, 0);
        thread::create("fs-init", thread::PRI_DEFAULT, fs_init_thread, core::ptr::null_mut());
        while !unsafe { FS_READY } {
            thread::yield_current();
        }
    } else {
        kprintln!("warning: no disk, filesystem unavailable");
    }

    // Launch /init (PID 1) -- the UNIX way
    let has_fs = devices::block::get_role(devices::block::BlockType::FileSys).is_some();
    if has_fs {
        let init_tid = userprog::process::execute("init");
        if init_tid != thread::TID_ERROR {
            kprintln!("init: pid {}", init_tid);
            let status = userprog::process::wait(init_tid);
            kprintln!("init exited ({})", status);
        } else {
            kprintln!("panic: cannot exec init");
        }
    }

    kprintln!("rxv6: halted");
    devices::shutdown::power_off()
}

static mut FS_READY: bool = false;

fn fs_init_thread(_aux: *mut u8) {
    let dev = devices::block::get_role(devices::block::BlockType::FileSys)
        .expect("fs-init: no FileSys device");
    kprintln!("fs: {} sectors", dev.size);
    filesys::filesys::init(false);
    kprintln!("fs: mounted");
    unsafe { FS_READY = true; }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::idt::intr_disable();
    devices::serial::write_fmt(format_args!("\n!!! KERNEL PANIC !!!\n{}\n", info));
    loop { unsafe { core::arch::asm!("cli", "hlt"); } }
}
