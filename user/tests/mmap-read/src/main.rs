#![no_std]
#![no_main]

use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-read) begin");

    syscall::create(b"mmap-r.dat\0", 0);
    let fd = syscall::open(b"mmap-r.dat\0");
    if fd < 0 {
        println!("(mmap-read) open failed");
        return 1;
    }
    syscall::write(fd, b"Hello mmap world!!");

    let mapid = syscall::mmap(fd, 0x10000000);
    if mapid <= 0 {
        println!("(mmap-read) mmap failed: {}", mapid);
        syscall::close(fd);
        return 1;
    }

    let ptr = 0x10000000 as *const u8;
    let data = unsafe { core::slice::from_raw_parts(ptr, 18) };
    if data == b"Hello mmap world!!" {
        println!("(mmap-read) PASSED");
    } else {
        println!("(mmap-read) FAILED: data mismatch");
    }

    syscall::munmap(mapid);
    syscall::close(fd);
    0
}
