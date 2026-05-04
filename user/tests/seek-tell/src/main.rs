#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(seek-tell) begin");
    syscall::create(b"st.txt\0", 0);
    let fd = syscall::open(b"st.txt\0");
    syscall::write(fd, b"0123456789");
    syscall::seek(fd, 5);
    let pos = syscall::tell(fd);
    let mut buf = [0u8; 5];
    let n = syscall::read(fd, &mut buf);
    let s = core::str::from_utf8(&buf[..n as usize]).unwrap_or("?");
    if pos == 5 && s == "56789" { println!("(seek-tell) PASSED"); } else { println!("(seek-tell) FAILED pos={} s={}", pos, s); }
    syscall::close(fd);
    0
}
