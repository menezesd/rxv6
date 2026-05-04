//! head - print first N lines of a file (default 10).

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let mut nlines: usize = 10;
    let mut file_start = 1;

    // Parse -N flag
    if argc > 1 {
        let arg1 = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
        if arg1.starts_with('-') {
            nlines = atoi(&arg1[1..]) as usize;
            if nlines == 0 { nlines = 10; }
            file_start = 2;
        }
    }

    if file_start >= argc as usize {
        head(0, nlines);
    } else {
        for i in file_start..(argc as usize) {
            let path = unsafe { *argv.add(i) };
            let fd = syscall::open(path, syscall::O_RDONLY);
            if fd < 0 {
                let name = unsafe { rxv6_user::cstr_to_str(path) };
                println!("head: cannot open {}", name);
                continue;
            }
            head(fd, nlines);
            syscall::close(fd);
        }
    }
    0
}

fn head(fd: i32, nlines: usize) {
    let mut lines_seen = 0usize;
    let mut buf = [0u8; 1];
    while lines_seen < nlines {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 { break; }
        syscall::write(1, &buf[..1]);
        if buf[0] == b'\n' { lines_seen += 1; }
    }
}

fn atoi(s: &str) -> i32 {
    let mut n: i32 = 0;
    for &b in s.as_bytes() {
        if b < b'0' || b > b'9' { break; }
        n = n * 10 + (b - b'0') as i32;
    }
    n
}
