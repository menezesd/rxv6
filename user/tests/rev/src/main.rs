//! rev - reverse lines of input.

#![no_std]
#![no_main]

use rxv6_user::syscall;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut line = [0u8; 1024];
    let mut len = 0;

    let mut buf = [0u8; 256];
    loop {
        let n = syscall::read(0, &mut buf);
        if n <= 0 {
            if len > 0 { reverse_write(&line, len); }
            break;
        }
        for i in 0..n as usize {
            if buf[i] == b'\n' {
                reverse_write(&line, len);
                syscall::write(1, b"\n");
                len = 0;
            } else if len < line.len() {
                line[len] = buf[i];
                len += 1;
            }
        }
    }
    0
}

fn reverse_write(line: &[u8], len: usize) {
    let mut rev = [0u8; 1024];
    for i in 0..len {
        rev[i] = line[len - 1 - i];
    }
    syscall::write(1, &rev[..len]);
}
