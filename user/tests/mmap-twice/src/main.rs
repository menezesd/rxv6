#![no_std]
#![no_main]

use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-twice) begin");

    syscall::create(b"mt.dat\0", 0);
    let fd = syscall::open(b"mt.dat\0");
    if fd < 0 {
        println!("(mmap-twice) open failed");
        return 1;
    }
    syscall::write(fd, b"double mapped");

    let m1 = syscall::mmap(fd, 0x10000000);
    let m2 = syscall::mmap(fd, 0x10010000);

    if m1 > 0 && m2 > 0 {
        let p1 = unsafe { core::slice::from_raw_parts(0x10000000 as *const u8, 13) };
        let p2 = unsafe { core::slice::from_raw_parts(0x10010000 as *const u8, 13) };
        if p1 == p2 && p1 == b"double mapped" {
            println!("(mmap-twice) PASSED");
        } else {
            println!("(mmap-twice) FAILED: data mismatch");
        }
    } else {
        println!("(mmap-twice) FAILED: mmap returned m1={}, m2={}", m1, m2);
    }

    if m1 > 0 { syscall::munmap(m1); }
    if m2 > 0 { syscall::munmap(m2); }
    syscall::close(fd);
    0
}
