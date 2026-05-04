//! wc - word, line, and byte count.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc <= 1 {
        wc(0, "");
    } else {
        for i in 1..argc {
            let path = unsafe { *argv.add(i as usize) };
            let fd = syscall::open(path, syscall::O_RDONLY);
            if fd < 0 {
                let name = unsafe { rxv6_user::cstr_to_str(path) };
                println!("wc: cannot open {}", name);
                continue;
            }
            let name = unsafe { rxv6_user::cstr_to_str(path) };
            wc(fd, name);
            syscall::close(fd);
        }
    }
    0
}

fn wc(fd: i32, name: &str) {
    let mut lines = 0u32;
    let mut words = 0u32;
    let mut bytes = 0u32;
    let mut in_word = false;
    let mut buf = [0u8; 512];

    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 { break; }
        for i in 0..n as usize {
            bytes += 1;
            if buf[i] == b'\n' { lines += 1; }
            if buf[i] == b' ' || buf[i] == b'\n' || buf[i] == b'\t' {
                in_word = false;
            } else if !in_word {
                words += 1;
                in_word = true;
            }
        }
    }
    println!("{} {} {} {}", lines, words, bytes, name);
}
