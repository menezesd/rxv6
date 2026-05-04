//! tee - copy stdin to stdout and a file.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: tee file");
        return 1;
    }
    let path = unsafe { *argv.add(1) };
    let fd = syscall::open(path, syscall::O_WRONLY | syscall::O_CREATE);
    if fd < 0 {
        println!("tee: cannot open file");
        return 1;
    }

    let mut buf = [0u8; 512];
    loop {
        let n = syscall::read(0, &mut buf);
        if n <= 0 { break; }
        let data = &buf[..n as usize];
        syscall::write(1, data);  // stdout
        syscall::write(fd, data); // file
    }

    syscall::close(fd);
    0
}
