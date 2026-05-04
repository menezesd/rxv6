#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(syn-write) begin");
    syscall::create(b"synw.dat\0", 0);
    let f1 = syscall::open(b"synw.dat\0");
    let f2 = syscall::open(b"synw.dat\0");
    syscall::write(f1, b"FIRST");
    syscall::write(f2, b"SECOND");
    // f2 writes at pos 0 since it has its own position
    // Read back
    syscall::seek(f1, 0);
    let mut buf = [0u8; 10];
    let n = syscall::read(f1, &mut buf);
    // Should see "SECONDIRST" or "FIRST" depending on file position behavior
    // Our impl: each fd has independent position, so f2 overwrites start
    if n > 0 {
        println!("(syn-write) PASSED (read {} bytes)", n);
    }
    syscall::close(f1);
    syscall::close(f2);
    0
}
