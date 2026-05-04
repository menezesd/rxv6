#![no_std]
#![no_main]

use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(mmap-null) begin");

    syscall::create(b"mn.dat\0", 0);
    let fd = syscall::open(b"mn.dat\0");
    if fd < 0 {
        println!("(mmap-null) open failed");
        return 1;
    }
    syscall::write(fd, b"data");

    let mapid = syscall::mmap(fd, 0);
    if mapid == -1 {
        println!("(mmap-null) PASSED");
    } else {
        println!("(mmap-null) FAILED: expected -1, got {}", mapid);
    }

    syscall::close(fd);
    0
}
