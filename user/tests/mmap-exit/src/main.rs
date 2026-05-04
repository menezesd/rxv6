#![no_std]
#![no_main]

use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-exit) begin");

    syscall::create(b"mmap-e.dat\0", 0);
    let fd = syscall::open(b"mmap-e.dat\0");
    if fd < 0 {
        println!("(mmap-exit) open failed");
        return 1;
    }
    let zeros = [0u8; 64];
    syscall::write(fd, &zeros);

    let _mapid = syscall::mmap(fd, 0x10000000);
    let ptr = 0x10000000 as *mut u8;
    unsafe { *ptr = 42; }

    // Exit without munmap - kernel should write back dirty pages
    println!("(mmap-exit) PASSED: exiting with dirty mmap");
    0
}
