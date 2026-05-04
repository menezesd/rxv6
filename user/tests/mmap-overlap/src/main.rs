#![no_std]
#![no_main]

use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-overlap) begin");

    syscall::create(b"mo.dat\0", 0);
    let fd = syscall::open(b"mo.dat\0");
    if fd < 0 {
        println!("(mmap-overlap) open failed");
        return 1;
    }
    // Write enough data to span at least one page
    let buf = [0u8; 64];
    syscall::write(fd, &buf);

    let m1 = syscall::mmap(fd, 0x10000000);
    let m2 = syscall::mmap(fd, 0x10000000); // overlap - should fail

    if m1 > 0 && m2 == -1 {
        println!("(mmap-overlap) PASSED");
    } else {
        println!("(mmap-overlap) FAILED: m1={}, m2={}", m1, m2);
    }

    syscall::munmap(m1);
    syscall::close(fd);
    0
}
