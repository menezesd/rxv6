//! cat - concatenate files and print to stdout.

#![no_std]
#![no_main]

use rxv6_user::syscall;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc <= 1 {
        // No args: copy stdin to stdout
        cat(0);
    } else {
        for i in 1..argc {
            let path = unsafe { *argv.add(i as usize) };
            let fd = syscall::open(path, syscall::O_RDONLY);
            if fd < 0 {
                let name = unsafe { rxv6_user::cstr_to_str(path) };
                rxv6_user::println!("cat: cannot open {}", name);
                return 1;
            }
            cat(fd);
            syscall::close(fd);
        }
    }
    0
}

fn cat(fd: i32) {
    let mut buf = [0u8; 512];
    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 { break; }
        syscall::write(1, &buf[..n as usize]);
    }
}
