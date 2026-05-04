//! grep - search for a pattern in files (simple substring match).

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: grep pattern [file ...]");
        return 1;
    }

    let pattern = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };

    if argc == 2 {
        grep(pattern, 0);
    } else {
        for i in 2..argc {
            let path = unsafe { *argv.add(i as usize) };
            let fd = syscall::open(path, syscall::O_RDONLY);
            if fd < 0 {
                let name = unsafe { rxv6_user::cstr_to_str(path) };
                println!("grep: cannot open {}", name);
                continue;
            }
            grep(pattern, fd);
            syscall::close(fd);
        }
    }
    0
}

fn grep(pattern: &str, fd: i32) {
    let mut buf = [0u8; 1024];
    let mut line_start = 0usize;
    let mut buf_len = 0usize;

    loop {
        let n = syscall::read(fd, &mut buf[buf_len..]);
        if n <= 0 && buf_len == line_start { break; }
        if n > 0 { buf_len += n as usize; }

        // Process complete lines
        while line_start < buf_len {
            let remaining = &buf[line_start..buf_len];
            if let Some(nl) = remaining.iter().position(|&b| b == b'\n') {
                let line = &buf[line_start..line_start + nl];
                if contains(line, pattern.as_bytes()) {
                    syscall::write(1, line);
                    syscall::write(1, b"\n");
                }
                line_start += nl + 1;
            } else if n <= 0 {
                // EOF with no newline: check last partial line
                let line = &buf[line_start..buf_len];
                if contains(line, pattern.as_bytes()) {
                    syscall::write(1, line);
                    syscall::write(1, b"\n");
                }
                line_start = buf_len;
            } else {
                break;
            }
        }

        // Shift remaining data to start of buffer
        if line_start > 0 {
            let remaining = buf_len - line_start;
            buf.copy_within(line_start..buf_len, 0);
            buf_len = remaining;
            line_start = 0;
        }

        if n <= 0 { break; }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() { return true; }
    if haystack.len() < needle.len() { return false; }
    for i in 0..=haystack.len() - needle.len() {
        if &haystack[i..i + needle.len()] == needle {
            return true;
        }
    }
    false
}
