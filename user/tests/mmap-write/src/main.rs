#![no_std]
#![no_main]

use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-write) begin");

    syscall::create(b"mmap-w.dat\0", 0);
    let fd = syscall::open(b"mmap-w.dat\0");
    if fd < 0 {
        println!("(mmap-write) open failed");
        return 1;
    }
    // Pre-fill with zeros so the file has content to map
    let zeros = [0u8; 64];
    syscall::write(fd, &zeros);

    let mapid = syscall::mmap(fd, 0x10000000);
    if mapid <= 0 {
        println!("(mmap-write) mmap failed: {}", mapid);
        syscall::close(fd);
        return 1;
    }

    // Write through the mapping
    let ptr = 0x10000000 as *mut u8;
    let msg = b"WRITTEN VIA MMAP!";
    unsafe {
        core::ptr::copy_nonoverlapping(msg.as_ptr(), ptr, msg.len());
    }

    // Munmap should write back dirty pages
    syscall::munmap(mapid);

    // Re-read from file to verify
    syscall::seek(fd, 0);
    let mut buf = [0u8; 17];
    let n = syscall::read(fd, &mut buf);
    if n == 17 && &buf == b"WRITTEN VIA MMAP!" {
        println!("(mmap-write) PASSED");
    } else {
        println!("(mmap-write) FAILED: data mismatch");
    }

    syscall::close(fd);
    0
}
