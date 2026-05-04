#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(lg-seq-block) begin");
    syscall::create(b"big.dat\0", 0);
    let fd = syscall::open(b"big.dat\0");
    if fd < 0 {
        println!("(lg-seq-block) FAILED - could not open file");
        return 1;
    }
    // Write 4KB sequentially in 512-byte chunks
    let data = [0xABu8; 512];
    for _ in 0..8 {
        syscall::write(fd, &data);
    }
    // Read back
    syscall::seek(fd, 0);
    let mut buf = [0u8; 512];
    let mut ok = true;
    for _ in 0..8 {
        let n = syscall::read(fd, &mut buf);
        if n != 512 || buf[0] != 0xAB {
            ok = false;
            break;
        }
    }
    syscall::close(fd);
    if ok {
        println!("(lg-seq-block) PASSED");
    } else {
        println!("(lg-seq-block) FAILED");
    }
    0
}
