#![no_std]
#![no_main]
extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    syscall::create(b"ssb.dat\0", 0);
    let fd = syscall::open(b"ssb.dat\0");
    let block = [0xBBu8; 128];
    for _ in 0..4 {
        syscall::write(fd, &block);
    }
    syscall::seek(fd, 0);
    let mut buf = [0u8; 128];
    let mut ok = true;
    for _ in 0..4 {
        let n = syscall::read(fd, &mut buf);
        if n != 128 || buf[0] != 0xBB {
            ok = false;
            break;
        }
    }
    if ok {
        println!("(sm-seq-block) PASSED");
    }
    syscall::close(fd);
    0
}
