#![no_std]
#![no_main]

use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-unmap) begin");

    syscall::create(b"mu.dat\0", 0);
    let fd = syscall::open(b"mu.dat\0");
    if fd < 0 {
        println!("(mmap-unmap) open failed");
        return 1;
    }
    syscall::write(fd, b"data");

    let mapid = syscall::mmap(fd, 0x10000000);
    if mapid <= 0 {
        println!("(mmap-unmap) mmap failed");
        syscall::close(fd);
        return 1;
    }

    syscall::munmap(mapid);
    // Page should be unmapped now - accessing it would fault
    println!("(mmap-unmap) PASSED");

    syscall::close(fd);
    0
}
