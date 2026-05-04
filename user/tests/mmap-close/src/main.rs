#![no_std]
#![no_main]

use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-close) begin");

    syscall::create(b"mmap-c.dat\0", 0);
    let fd = syscall::open(b"mmap-c.dat\0");
    if fd < 0 {
        println!("(mmap-close) open failed");
        return 1;
    }
    syscall::write(fd, b"still accessible!");

    let mapid = syscall::mmap(fd, 0x10000000);
    if mapid <= 0 {
        println!("(mmap-close) mmap failed");
        syscall::close(fd);
        return 1;
    }

    // Close the fd - mapping should survive
    syscall::close(fd);

    let ptr = 0x10000000 as *const u8;
    let data = unsafe { core::slice::from_raw_parts(ptr, 17) };
    if data == b"still accessible!" {
        println!("(mmap-close) PASSED");
    } else {
        println!("(mmap-close) FAILED: data mismatch after fd close");
    }

    syscall::munmap(mapid);
    0
}
