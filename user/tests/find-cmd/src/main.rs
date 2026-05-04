//! find - search for files in a directory hierarchy.
//!
//! Usage: find [path] [-name pattern] [-type f|d]
//! Simplified: only supports root-level search (no deep recursion due to
//! rxv6 flat directory structure). Pattern supports leading/trailing *.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let mut dir = ".";
    let mut name_pattern: Option<&str> = None;
    let mut type_filter: Option<u8> = None; // b'f' or b'd'

    let mut i = 1;
    while i < argc as usize {
        let arg = unsafe { rxv6_user::cstr_to_str(*argv.add(i)) };
        if arg == "-name" && i + 1 < argc as usize {
            i += 1;
            name_pattern = Some(unsafe { rxv6_user::cstr_to_str(*argv.add(i)) });
        } else if arg == "-type" && i + 1 < argc as usize {
            i += 1;
            let t = unsafe { rxv6_user::cstr_to_str(*argv.add(i)) };
            if !t.is_empty() {
                type_filter = Some(t.as_bytes()[0]);
            }
        } else if !arg.starts_with('-') {
            dir = arg;
        }
        i += 1;
    }

    find_in(dir, name_pattern, type_filter);
    0
}

fn find_in(dir: &str, name_pattern: Option<&str>, type_filter: Option<u8>) {
    let mut path_buf = [0u8; 128];
    let len = dir.len().min(127);
    path_buf[..len].copy_from_slice(&dir.as_bytes()[..len]);

    let fd = syscall::open(path_buf.as_ptr(), syscall::O_RDONLY);
    if fd < 0 { return; }

    // Check if it's a directory
    let mut st = syscall::Stat {
        file_type: syscall::FileType::None,
        dev: 0, ino: 0, nlink: 0, size: 0,
    };
    syscall::fstat(fd, &mut st);

    if st.file_type != syscall::FileType::Dir {
        // It's a file — check and print
        if matches_filter(dir, name_pattern, type_filter, false) {
            println!("{}", dir);
        }
        syscall::close(fd);
        return;
    }

    // Read directory entries (xv6 format: 16 bytes each: 2-byte inum + 14-byte name)
    let mut buf = [0u8; 256];
    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 { break; }
        let mut off = 0;
        while off + 16 <= n as usize {
            let inum = u16::from_le_bytes([buf[off], buf[off + 1]]);
            if inum == 0 { off += 16; continue; }

            // Extract name (up to 14 bytes, null-terminated)
            let name_bytes = &buf[off + 2..off + 16];
            let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(14);
            let name = unsafe { core::str::from_utf8_unchecked(&name_bytes[..name_len]) };

            // Skip . and ..
            if name == "." || name == ".." {
                off += 16;
                continue;
            }

            // Build full path
            let full = if dir == "." {
                name
            } else {
                // Can't easily concat without alloc; just print dir/name
                // Use a static buf trick
                off += 16;
                print_path(dir, name, name_pattern, type_filter);
                continue;
            };

            // Stat this entry to determine type
            let is_dir = check_is_dir(full);
            if matches_filter(full, name_pattern, type_filter, is_dir) {
                println!("{}", full);
            }
            off += 16;
        }
    }
    syscall::close(fd);
}

fn print_path(dir: &str, name: &str, name_pattern: Option<&str>, type_filter: Option<u8>) {
    // Build path: dir/name
    let mut pbuf = [0u8; 128];
    let dlen = dir.len().min(100);
    pbuf[..dlen].copy_from_slice(&dir.as_bytes()[..dlen]);
    pbuf[dlen] = b'/';
    let nlen = name.len().min(126 - dlen);
    pbuf[dlen + 1..dlen + 1 + nlen].copy_from_slice(&name.as_bytes()[..nlen]);
    let total = dlen + 1 + nlen;
    let path = unsafe { core::str::from_utf8_unchecked(&pbuf[..total]) };

    let is_dir = check_is_dir(path);
    if matches_filter(name, name_pattern, type_filter, is_dir) {
        println!("{}", path);
    }
}

fn check_is_dir(path: &str) -> bool {
    let mut buf = [0u8; 128];
    let len = path.len().min(127);
    buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    let fd = syscall::open(buf.as_ptr(), syscall::O_RDONLY);
    if fd < 0 { return false; }
    let mut st = syscall::Stat {
        file_type: syscall::FileType::None,
        dev: 0, ino: 0, nlink: 0, size: 0,
    };
    syscall::fstat(fd, &mut st);
    syscall::close(fd);
    st.file_type == syscall::FileType::Dir
}

fn matches_filter(name: &str, pattern: Option<&str>, type_filter: Option<u8>, is_dir: bool) -> bool {
    // Type filter
    if let Some(t) = type_filter {
        match t {
            b'f' if is_dir => return false,
            b'd' if !is_dir => return false,
            _ => {}
        }
    }
    // Name pattern (simple glob: leading *, trailing *, or exact)
    if let Some(pat) = pattern {
        let base = name.rsplit('/').next().unwrap_or(name);
        return simple_glob(pat, base);
    }
    true
}

fn simple_glob(pattern: &str, name: &str) -> bool {
    let p = pattern.as_bytes();
    if p.is_empty() { return name.is_empty(); }
    if p == b"*" { return true; }
    if p[0] == b'*' && p[p.len() - 1] == b'*' {
        // *middle* — contains
        let mid = unsafe { core::str::from_utf8_unchecked(&p[1..p.len() - 1]) };
        return name.contains(mid);
    }
    if p[0] == b'*' {
        // *suffix
        let suffix = unsafe { core::str::from_utf8_unchecked(&p[1..]) };
        return name.ends_with(suffix);
    }
    if p[p.len() - 1] == b'*' {
        // prefix*
        let prefix = unsafe { core::str::from_utf8_unchecked(&p[..p.len() - 1]) };
        return name.starts_with(prefix);
    }
    // Exact match
    pattern == name
}
