//! cp - copy files.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 3 {
        println!("usage: cp source dest");
        return 1;
    }
    let src = unsafe { *argv.add(1) };
    let dst = unsafe { *argv.add(2) };

    let sfd = syscall::open(src, syscall::O_RDONLY);
    if sfd < 0 {
        println!("cp: cannot open source");
        return 1;
    }
    let dfd = syscall::open(dst, syscall::O_WRONLY | syscall::O_CREATE);
    if dfd < 0 {
        println!("cp: cannot create dest");
        syscall::close(sfd);
        return 1;
    }

    let mut buf = [0u8; 512];
    loop {
        let n = syscall::read(sfd, &mut buf);
        if n <= 0 { break; }
        syscall::write(dfd, &buf[..n as usize]);
    }

    syscall::close(sfd);
    syscall::close(dfd);
    0
}
