//! ed - the standard UNIX line editor.
//!
//! Commands:
//!   (empty)    - print current line number
//!   .          - current line number
//!   NUMBER     - go to line and print it
//!   $          - go to last line and print it
//!   p          - print current line
//!   NUMBERp    - print line NUMBER
//!   N1,N2p     - print lines N1 through N2
//!   ,p         - print all lines (same as 1,$p)
//!   a          - append after current line (enter text, "." on a line to stop)
//!   i          - insert before current line
//!   d          - delete current line
//!   N1,N2d     - delete lines N1 through N2
//!   c          - change current line (delete + insert)
//!   s/old/new/ - substitute first occurrence on current line
//!   w          - write to file
//!   w FILE     - write to FILE
//!   e FILE     - edit FILE (load into buffer)
//!   q          - quit
//!   Q          - quit without checking for unsaved changes

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

const MAX_LINES: usize = 1024;
const MAX_LINE_LEN: usize = 256;
const CMD_BUF: usize = 256;

struct Buffer {
    lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    lens: [usize; MAX_LINES],
    count: usize,
    current: usize, // 1-based, 0 means empty buffer
    dirty: bool,
    filename: [u8; 64],
    filename_len: usize,
}

impl Buffer {
    fn new() -> Self {
        // Safety: all fields are plain data, zero-init is valid
        unsafe { core::mem::zeroed() }
    }
}

static mut BUF: Buffer = unsafe { core::mem::zeroed() };

fn buf() -> &'static mut Buffer {
    unsafe { &mut *core::ptr::addr_of_mut!(BUF) }
}

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let b = buf();
    *b = Buffer::new();

    if argc > 1 {
        let path = unsafe { *argv.add(1) };
        set_filename(b, path);
        load_file(b);
    }

    let mut cmd = [0u8; CMD_BUF];
    loop {
        let n = getline(&mut cmd);
        if n <= 0 { break; }
        let len = cmd[..n as usize].iter().position(|&c| c == b'\n' || c == 0).unwrap_or(n as usize);
        let line = &cmd[..len];
        if line.is_empty() {
            if b.count > 0 { println!("{}", b.current); }
            continue;
        }
        if !execute_cmd(b, line) { break; }
    }
    0
}

fn execute_cmd(b: &mut Buffer, cmd: &[u8]) -> bool {
    let len = cmd.len();
    if len == 0 { return true; }

    // q / Q - quit
    if cmd == b"q" {
        if b.dirty {
            println!("?"); // warning: unsaved changes
            b.dirty = false; // next q will quit
            return true;
        }
        return false;
    }
    if cmd == b"Q" { return false; }

    // w [file] - write
    if cmd[0] == b'w' {
        let fname = if len > 2 && cmd[1] == b' ' { Some(&cmd[2..]) } else { None };
        write_file(b, fname);
        return true;
    }

    // e file - edit
    if cmd[0] == b'e' && len > 2 && cmd[1] == b' ' {
        set_filename_bytes(b, &cmd[2..]);
        load_file(b);
        return true;
    }

    // a - append
    if cmd == b"a" {
        input_mode(b, b.current + 1);
        return true;
    }

    // i - insert
    if cmd == b"i" {
        let pos = if b.current > 0 { b.current } else { 1 };
        input_mode(b, pos);
        return true;
    }

    // s/old/new/ - substitute
    if cmd[0] == b's' && len > 1 && cmd[1] == b'/' {
        substitute(b, &cmd[2..]);
        return true;
    }

    // Parse address(es) and command
    let (addr1, addr2, rest) = parse_range(b, cmd);

    if rest.is_empty() {
        // Just an address — go to line and print
        let line = if addr2 > 0 { addr2 } else { addr1 };
        if line >= 1 && line <= b.count {
            b.current = line;
            print_line(b, line);
        } else if line > 0 {
            println!("?");
        }
        return true;
    }

    match rest[0] {
        b'p' => {
            let start = if addr1 > 0 { addr1 } else { b.current };
            let end = if addr2 > 0 { addr2 } else { start };
            for i in start..=end.min(b.count) {
                print_line(b, i);
            }
            if end <= b.count { b.current = end; }
        }
        b'd' => {
            let start = if addr1 > 0 { addr1 } else { b.current };
            let end = if addr2 > 0 { addr2 } else { start };
            delete_lines(b, start, end);
        }
        b'c' => {
            let line = if addr1 > 0 { addr1 } else { b.current };
            if line >= 1 && line <= b.count {
                delete_lines(b, line, line);
                input_mode(b, line);
            }
        }
        b'a' => {
            let line = if addr1 > 0 { addr1 } else { b.current };
            input_mode(b, line + 1);
        }
        b'i' => {
            let line = if addr1 > 0 { addr1 } else { b.current };
            input_mode(b, if line > 0 { line } else { 1 });
        }
        _ => { println!("?"); }
    }
    true
}

fn parse_range<'a>(b: &Buffer, cmd: &'a [u8]) -> (usize, usize, &'a [u8]) {
    let mut i = 0;
    let len = cmd.len();

    // ,p means 1,$p
    if cmd[0] == b',' {
        let rest = &cmd[1..];
        return (1, b.count, rest);
    }

    // Parse first address
    let addr1 = if i < len && cmd[i] == b'$' {
        i += 1;
        b.count
    } else if i < len && cmd[i] == b'.' {
        i += 1;
        b.current
    } else {
        let (n, consumed) = parse_num(&cmd[i..]);
        i += consumed;
        n
    };

    // Check for comma (range)
    let addr2 = if i < len && cmd[i] == b',' {
        i += 1;
        if i < len && cmd[i] == b'$' {
            i += 1;
            b.count
        } else {
            let (n, consumed) = parse_num(&cmd[i..]);
            i += consumed;
            if n > 0 { n } else { b.count }
        }
    } else {
        0
    };

    (addr1, addr2, &cmd[i..])
}

fn parse_num(s: &[u8]) -> (usize, usize) {
    let mut n = 0usize;
    let mut i = 0;
    while i < s.len() && s[i] >= b'0' && s[i] <= b'9' {
        n = n * 10 + (s[i] - b'0') as usize;
        i += 1;
    }
    (n, i)
}

fn input_mode(b: &mut Buffer, at: usize) {
    let mut insert_at = at;
    let mut line_buf = [0u8; MAX_LINE_LEN];
    loop {
        let n = getline(&mut line_buf);
        if n <= 0 { break; }
        let len = n as usize;
        // Check for "." on a line by itself
        if (len == 1 && line_buf[0] == b'.') || (len == 2 && line_buf[0] == b'.' && line_buf[1] == b'\n') {
            break;
        }
        if b.count >= MAX_LINES {
            println!("?"); // buffer full
            break;
        }
        // Remove trailing newline
        let text_len = if len > 0 && line_buf[len - 1] == b'\n' { len - 1 } else { len };
        // Shift lines down to make room
        if insert_at <= b.count {
            for j in (insert_at..=b.count).rev() {
                if j < MAX_LINES {
                    b.lines[j] = b.lines[j - 1];
                    b.lens[j] = b.lens[j - 1];
                }
            }
        }
        let idx = insert_at - 1; // 0-based
        if idx < MAX_LINES {
            let copy_len = text_len.min(MAX_LINE_LEN);
            b.lines[idx][..copy_len].copy_from_slice(&line_buf[..copy_len]);
            b.lens[idx] = copy_len;
        }
        b.count += 1;
        b.current = insert_at;
        b.dirty = true;
        insert_at += 1;
    }
}

fn delete_lines(b: &mut Buffer, start: usize, end: usize) {
    if start < 1 || start > b.count || end < start { return; }
    let end = end.min(b.count);
    let removed = end - start + 1;
    for i in (start - 1)..(b.count - removed) {
        b.lines[i] = b.lines[i + removed];
        b.lens[i] = b.lens[i + removed];
    }
    b.count -= removed;
    b.current = if start <= b.count { start } else { b.count };
    b.dirty = true;
}

fn substitute(b: &mut Buffer, cmd: &[u8]) {
    if b.current < 1 || b.current > b.count { println!("?"); return; }

    // Parse s/old/new/
    let sep = b'/';
    let parts: [&[u8]; 2] = {
        let mut p = [&[][..]; 2];
        let mut start = 0;
        let mut pi = 0;
        for i in 0..cmd.len() {
            if cmd[i] == sep && pi < 2 {
                p[pi] = &cmd[start..i];
                pi += 1;
                start = i + 1;
            }
        }
        if pi < 2 { p[pi] = &cmd[start..]; }
        p
    };

    let old = parts[0];
    let new = parts[1];
    if old.is_empty() { return; }

    let idx = b.current - 1;
    let line = &b.lines[idx][..b.lens[idx]];

    // Find old in line
    if let Some(pos) = find_substr(line, old) {
        let mut result = [0u8; MAX_LINE_LEN];
        let mut ri = 0;
        // Copy before match
        result[..pos].copy_from_slice(&line[..pos]);
        ri += pos;
        // Copy replacement
        let nlen = new.len().min(MAX_LINE_LEN - ri);
        result[ri..ri + nlen].copy_from_slice(&new[..nlen]);
        ri += nlen;
        // Copy after match
        let after = pos + old.len();
        let rest = (b.lens[idx] - after).min(MAX_LINE_LEN - ri);
        result[ri..ri + rest].copy_from_slice(&line[after..after + rest]);
        ri += rest;

        b.lines[idx][..ri].copy_from_slice(&result[..ri]);
        b.lens[idx] = ri;
        b.dirty = true;
        print_line(b, b.current);
    }
}

fn find_substr(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() { return None; }
    for i in 0..=haystack.len() - needle.len() {
        if &haystack[i..i + needle.len()] == needle { return Some(i); }
    }
    None
}

fn print_line(b: &Buffer, num: usize) {
    if num < 1 || num > b.count { return; }
    let idx = num - 1;
    let s = &b.lines[idx][..b.lens[idx]];
    syscall::write(1, s);
    syscall::write(1, b"\n");
}

fn load_file(b: &mut Buffer) {
    if b.filename_len == 0 { println!("?"); return; }
    let fd = syscall::open(b.filename.as_ptr(), syscall::O_RDONLY);
    if fd < 0 {
        // New file
        b.count = 0;
        b.current = 0;
        b.dirty = false;
        println!("(new file)");
        return;
    }

    b.count = 0;
    b.current = 0;
    let mut total_bytes = 0u32;
    let mut byte_buf = [0u8; 1];
    let mut line_buf = [0u8; MAX_LINE_LEN];
    let mut li = 0;

    loop {
        let n = syscall::read(fd, &mut byte_buf);
        if n <= 0 {
            // Flush remaining partial line
            if li > 0 && b.count < MAX_LINES {
                b.lines[b.count][..li].copy_from_slice(&line_buf[..li]);
                b.lens[b.count] = li;
                b.count += 1;
                total_bytes += li as u32;
            }
            break;
        }
        total_bytes += 1;
        if byte_buf[0] == b'\n' {
            if b.count < MAX_LINES {
                b.lines[b.count][..li].copy_from_slice(&line_buf[..li]);
                b.lens[b.count] = li;
                b.count += 1;
            }
            li = 0;
        } else {
            if li < MAX_LINE_LEN {
                line_buf[li] = byte_buf[0];
                li += 1;
            }
        }
    }

    syscall::close(fd);
    b.current = if b.count > 0 { b.count } else { 0 };
    b.dirty = false;
    println!("{}", total_bytes);
}

fn write_file(b: &mut Buffer, fname: Option<&[u8]>) {
    if let Some(f) = fname {
        set_filename_bytes(b, f);
    }
    if b.filename_len == 0 { println!("?"); return; }

    let fd = syscall::open(b.filename.as_ptr(), syscall::O_WRONLY | syscall::O_CREATE);
    if fd < 0 { println!("?"); return; }

    let mut total = 0u32;
    for i in 0..b.count {
        let n = syscall::write(fd, &b.lines[i][..b.lens[i]]);
        if n > 0 { total += n as u32; }
        syscall::write(fd, b"\n");
        total += 1;
    }
    syscall::close(fd);
    b.dirty = false;
    println!("{}", total);
}

fn set_filename(b: &mut Buffer, ptr: *const u8) {
    let s = unsafe { rxv6_user::cstr_to_str(ptr) };
    set_filename_bytes(b, s.as_bytes());
}

fn set_filename_bytes(b: &mut Buffer, name: &[u8]) {
    let len = name.len().min(63);
    b.filename[..len].copy_from_slice(&name[..len]);
    b.filename[len] = 0;
    b.filename_len = len;
}

fn getline(buf: &mut [u8]) -> i32 {
    let mut i = 0;
    while i < buf.len() - 1 {
        let mut c = [0u8; 1];
        let n = syscall::read(0, &mut c);
        if n <= 0 { break; }
        buf[i] = c[0];
        i += 1;
        if c[0] == b'\n' { break; }
    }
    buf[i] = 0;
    i as i32
}
