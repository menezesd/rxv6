//! tail - print last N lines (default 10).
#![no_std]
#![no_main]
use rxv6_user::syscall;
use rxv6_user::println;

const MAX_LINES: usize = 256;
const MAX_LINE: usize = 128;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let mut nlines: usize = 10;
    let mut file_arg = 1;
    if argc > 1 {
        let a = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
        if a.starts_with('-') { nlines = atoi(&a[1..]); file_arg = 2; }
    }
    let fd = if file_arg < argc as usize {
        let p = unsafe { *argv.add(file_arg) };
        let f = syscall::open(p, syscall::O_RDONLY);
        if f < 0 { println!("tail: cannot open"); return 1; }
        f
    } else { 0 };

    // Read all lines into ring buffer
    let mut ring: [[u8; MAX_LINE]; MAX_LINES] = [[0; MAX_LINE]; MAX_LINES];
    let mut rlens: [usize; MAX_LINES] = [0; MAX_LINES];
    let mut total = 0usize;
    let mut ci = 0usize;
    let mut cur = [0u8; MAX_LINE];
    let mut buf = [0u8; 512];
    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 { if ci > 0 { let idx = total % MAX_LINES; ring[idx][..ci].copy_from_slice(&cur[..ci]); rlens[idx] = ci; total += 1; } break; }
        for i in 0..n as usize {
            if buf[i] == b'\n' { let idx = total % MAX_LINES; ring[idx][..ci].copy_from_slice(&cur[..ci]); rlens[idx] = ci; total += 1; ci = 0; }
            else if ci < MAX_LINE { cur[ci] = buf[i]; ci += 1; }
        }
    }
    if fd > 0 { syscall::close(fd); }

    let start = if total > nlines { total - nlines } else { 0 };
    for i in start..total {
        let idx = i % MAX_LINES;
        syscall::write(1, &ring[idx][..rlens[idx]]);
        syscall::write(1, b"\n");
    }
    0
}
fn atoi(s: &str) -> usize { let mut n = 0; for &b in s.as_bytes() { if b < b'0' || b > b'9' { break; } n = n*10+(b-b'0') as usize; } n }
