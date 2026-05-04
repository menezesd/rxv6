#![no_std]
#![no_main]

use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-clean) begin");

    syscall::create(b"mc.dat\0", 0);
    let fd = syscall::open(b"mc.dat\0");
    if fd < 0 {
        println!("(mmap-clean) open failed");
        return 1;
    }
    syscall::write(fd, b"clean read");

    let mapid = syscall::mmap(fd, 0x10000000);
    if mapid <= 0 {
        println!("(mmap-clean) mmap failed");
        syscall::close(fd);
        return 1;
    }

    let ptr = 0x10000000 as *const u8;
    let b = unsafe { *ptr };
    if b == b'c' {
        println!("(mmap-clean) PASSED");
    } else {
        println!("(mmap-clean) FAILED: expected 'c', got {}", b);
    }

    syscall::munmap(mapid);
    syscall::close(fd);
    0
}
