//! sort - sort lines of text.
#![no_std]
#![no_main]
use rxv6_user::syscall;

const MAX_LINES: usize = 512;
const MAX_LINE: usize = 128;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let fd = if argc > 1 {
        let path = unsafe { *argv.add(1) };
        let f = syscall::open(path, syscall::O_RDONLY);
        if f < 0 { rxv6_user::println!("sort: cannot open file"); return 1; }
        f
    } else { 0 };

    let mut lines: [[u8; MAX_LINE]; MAX_LINES] = [[0; MAX_LINE]; MAX_LINES];
    let mut lens: [usize; MAX_LINES] = [0; MAX_LINES];
    let mut count = 0usize;
    let mut cur = [0u8; MAX_LINE];
    let mut ci = 0;
    let mut buf = [0u8; 512];
    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 {
            if ci > 0 && count < MAX_LINES { lines[count][..ci].copy_from_slice(&cur[..ci]); lens[count] = ci; count += 1; }
            break;
        }
        for i in 0..n as usize {
            if buf[i] == b'\n' {
                if count < MAX_LINES { lines[count][..ci].copy_from_slice(&cur[..ci]); lens[count] = ci; count += 1; }
                ci = 0;
            } else if ci < MAX_LINE { cur[ci] = buf[i]; ci += 1; }
        }
    }
    if fd > 0 { syscall::close(fd); }

    // Insertion sort
    for i in 1..count {
        let mut j = i;
        while j > 0 && cmp(&lines[j], lens[j], &lines[j-1], lens[j-1]) == core::cmp::Ordering::Less {
            let tmp = lines[j]; lines[j] = lines[j-1]; lines[j-1] = tmp;
            let tl = lens[j]; lens[j] = lens[j-1]; lens[j-1] = tl;
            j -= 1;
        }
    }

    for i in 0..count {
        syscall::write(1, &lines[i][..lens[i]]);
        syscall::write(1, b"\n");
    }
    0
}

fn cmp(a: &[u8], al: usize, b: &[u8], bl: usize) -> core::cmp::Ordering {
    let min = al.min(bl);
    for i in 0..min {
        if a[i] < b[i] { return core::cmp::Ordering::Less; }
        if a[i] > b[i] { return core::cmp::Ordering::Greater; }
    }
    al.cmp(&bl)
}
