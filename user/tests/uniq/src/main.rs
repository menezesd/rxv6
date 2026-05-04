//! uniq - filter adjacent duplicate lines.
#![no_std]
#![no_main]
use rxv6_user::syscall;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let fd = if argc > 1 {
        let path = unsafe { *argv.add(1) };
        let f = syscall::open(path, syscall::O_RDONLY);
        if f < 0 { rxv6_user::println!("uniq: cannot open file"); return 1; }
        f
    } else { 0 };

    let mut prev = [0u8; 256];
    let mut prev_len = 0usize;
    let mut cur = [0u8; 256];
    let mut ci = 0usize;
    let mut buf = [0u8; 512];
    let mut first = true;

    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 {
            if ci > 0 && (first || &cur[..ci] != &prev[..prev_len]) {
                syscall::write(1, &cur[..ci]); syscall::write(1, b"\n");
            }
            break;
        }
        for i in 0..n as usize {
            if buf[i] == b'\n' {
                if first || &cur[..ci] != &prev[..prev_len] {
                    syscall::write(1, &cur[..ci]); syscall::write(1, b"\n");
                    prev[..ci].copy_from_slice(&cur[..ci]);
                    prev_len = ci;
                    first = false;
                }
                ci = 0;
            } else if ci < 255 { cur[ci] = buf[i]; ci += 1; }
        }
    }
    if fd > 0 { syscall::close(fd); }
    0
}
