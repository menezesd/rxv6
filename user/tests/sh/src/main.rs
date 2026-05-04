//! sh - a simple UNIX shell with command history.
//!
//! Supports:
//! - Simple commands: cmd arg1 arg2
//! - Pipes: cmd1 | cmd2
//! - Output redirection: cmd > file
//! - Input redirection: cmd < file
//! - Background: cmd &
//! - Built-ins: cd dir, exit [n], clear
//! - Command history: up/down arrows recall previous commands

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

const MAXARGS: usize = 16;
const BUFSIZE: usize = 256;
const HISTORY_SIZE: usize = 32;

struct History {
    lines: [[u8; BUFSIZE]; HISTORY_SIZE],
    lens: [usize; HISTORY_SIZE],
    count: usize,     // total lines added
    browse: usize,    // current browse position (index into ring)
}

impl History {
    const fn new() -> Self {
        History {
            lines: [[0; BUFSIZE]; HISTORY_SIZE],
            lens: [0; HISTORY_SIZE],
            count: 0,
            browse: 0,
        }
    }

    fn push(&mut self, line: &[u8], len: usize) {
        if len == 0 { return; }
        let idx = self.count % HISTORY_SIZE;
        self.lines[idx][..len].copy_from_slice(&line[..len]);
        self.lines[idx][len] = 0;
        self.lens[idx] = len;
        self.count += 1;
        self.browse = self.count;
    }

    fn total(&self) -> usize { self.count }

    fn get(&self, idx: usize) -> Option<(&[u8], usize)> {
        if idx >= self.count { return None; }
        let oldest = if self.count > HISTORY_SIZE { self.count - HISTORY_SIZE } else { 0 };
        if idx < oldest { return None; }
        let slot = idx % HISTORY_SIZE;
        Some((&self.lines[slot], self.lens[slot]))
    }
}

static mut HIST: History = History::new();

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut buf = [0u8; BUFSIZE];

    loop {
        print!("$ ");
        let n = getline_with_history(&mut buf);
        if n <= 0 {
            println!("");
            break;
        }
        let len = n as usize;
        // Remove trailing newline for processing
        let cmdlen = if len > 0 && buf[len - 1] == b'\n' { len - 1 } else { len };
        buf[cmdlen] = 0;

        let line = &buf[..cmdlen];
        if line.is_empty() || line[0] == 0 {
            continue;
        }

        // Save to history
        unsafe { HIST.push(line, cmdlen); }

        // Built-in: exit
        if starts_with(line, b"exit") {
            let rest = &line[4..];
            if rest.is_empty() || rest[0] == b' ' || rest[0] == 0 {
                let code = parse_exit_code(rest);
                syscall::exit(code);
            }
        }

        // Built-in: cd
        if starts_with(line, b"cd ") {
            let dir = &line[3..];
            let end = dir.iter().position(|&b| b == 0 || b == b' ').unwrap_or(dir.len());
            let mut path_buf = [0u8; 128];
            path_buf[..end].copy_from_slice(&dir[..end]);
            path_buf[end] = 0;
            if syscall::chdir(path_buf.as_ptr()) < 0 {
                println!("cd: cannot cd {}", unsafe { core::str::from_utf8_unchecked(&path_buf[..end]) });
            }
            continue;
        }

        // Built-in: clear
        if line == b"clear" || starts_with(line, b"clear\0") {
            syscall::write(1, b"\x1b[2J\x1b[H");
            continue;
        }

        // Fork and execute
        let pid = syscall::fork();
        if pid < 0 {
            println!("sh: fork failed");
            continue;
        }
        if pid == 0 {
            run_cmd(line);
            syscall::exit(0);
        }
        let bg = has_ampersand(line);
        if !bg {
            let mut status: i32 = 0;
            syscall::wait(&mut status as *mut i32);
        }
    }
    0
}

fn parse_exit_code(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && s[i] == b' ' { i += 1; }
    if i >= s.len() || s[i] == 0 { return 0; }
    let mut n: i32 = 0;
    let neg = s[i] == b'-';
    if neg { i += 1; }
    while i < s.len() && s[i] >= b'0' && s[i] <= b'9' {
        n = n * 10 + (s[i] - b'0') as i32;
        i += 1;
    }
    if neg { -n } else { n }
}

/// Read a line with history support using raw mode.
fn getline_with_history(buf: &mut [u8]) -> i32 {
    syscall::set_raw_mode(true);
    unsafe { HIST.browse = HIST.total(); }
    let mut pos = 0usize; // current cursor position in buf
    let mut len = 0usize; // total characters in buf

    loop {
        let mut c = [0u8; 1];
        let n = syscall::read(0, &mut c);
        if n <= 0 {
            syscall::set_raw_mode(false);
            return -1;
        }

        match c[0] {
            b'\n' | b'\r' => {
                syscall::write(1, b"\n");
                buf[len] = b'\n';
                len += 1;
                buf[len] = 0;
                syscall::set_raw_mode(false);
                return len as i32;
            }
            0x7F | 0x08 => {
                // Backspace
                if pos > 0 {
                    // Remove char at pos-1, shift rest left
                    for i in pos..len {
                        buf[i - 1] = buf[i];
                    }
                    pos -= 1;
                    len -= 1;
                    // Redraw from cursor position
                    syscall::write(1, b"\x08"); // move back
                    syscall::write(1, &buf[pos..len]);
                    syscall::write(1, b" "); // erase last char
                    // Move cursor back to pos
                    let back = len - pos + 1;
                    for _ in 0..back {
                        syscall::write(1, b"\x08");
                    }
                }
            }
            0x15 => {
                // Ctrl-U: kill line
                while pos > 0 {
                    syscall::write(1, b"\x08 \x08");
                    pos -= 1;
                }
                len = 0;
            }
            0x01 => {
                // Ctrl-A: beginning of line
                while pos > 0 {
                    syscall::write(1, b"\x08");
                    pos -= 1;
                }
            }
            0x05 => {
                // Ctrl-E: end of line
                if pos < len {
                    syscall::write(1, &buf[pos..len]);
                    pos = len;
                }
            }
            0x04 => {
                // Ctrl-D: EOF if empty
                if len == 0 {
                    syscall::set_raw_mode(false);
                    return 0;
                }
            }
            0x1B => {
                // Escape sequence
                let mut seq = [0u8; 2];
                let n1 = syscall::read(0, &mut seq[..1]);
                if n1 <= 0 { continue; }
                if seq[0] == b'[' {
                    let n2 = syscall::read(0, &mut seq[1..2]);
                    if n2 <= 0 { continue; }
                    match seq[1] {
                        b'A' => {
                            // Up arrow: previous history
                            unsafe {
                                if HIST.browse > 0 {
                                    let oldest = if HIST.count > HISTORY_SIZE { HIST.count - HISTORY_SIZE } else { 0 };
                                    if HIST.browse > oldest {
                                        HIST.browse -= 1;
                                        replace_line(buf, &mut pos, &mut len, &HIST);
                                    }
                                }
                            }
                        }
                        b'B' => {
                            // Down arrow: next history
                            unsafe {
                                if HIST.browse < HIST.total() {
                                    HIST.browse += 1;
                                    replace_line(buf, &mut pos, &mut len, &HIST);
                                }
                            }
                        }
                        b'C' => {
                            // Right arrow
                            if pos < len {
                                syscall::write(1, b"\x1b[C");
                                pos += 1;
                            }
                        }
                        b'D' => {
                            // Left arrow
                            if pos > 0 {
                                syscall::write(1, b"\x1b[D");
                                pos -= 1;
                            }
                        }
                        b'H' => {
                            // Home
                            while pos > 0 {
                                syscall::write(1, b"\x08");
                                pos -= 1;
                            }
                        }
                        b'F' => {
                            // End
                            if pos < len {
                                syscall::write(1, &buf[pos..len]);
                                pos = len;
                            }
                        }
                        b'3' => {
                            // Possibly Delete key: ESC[3~
                            let mut tilde = [0u8; 1];
                            let _ = syscall::read(0, &mut tilde);
                            if tilde[0] == b'~' && pos < len {
                                for i in pos..len - 1 {
                                    buf[i] = buf[i + 1];
                                }
                                len -= 1;
                                syscall::write(1, &buf[pos..len]);
                                syscall::write(1, b" ");
                                let back = len - pos + 1;
                                for _ in 0..back {
                                    syscall::write(1, b"\x08");
                                }
                            }
                        }
                        _ => {
                            // Consume trailing ~ for sequences like ESC[5~
                            if seq[1] >= b'0' && seq[1] <= b'9' {
                                let mut tilde = [0u8; 1];
                                let _ = syscall::read(0, &mut tilde);
                            }
                        }
                    }
                }
            }
            c if c >= 0x20 => {
                // Printable character
                if len < buf.len() - 2 {
                    // Insert at pos
                    for i in (pos..len).rev() {
                        buf[i + 1] = buf[i];
                    }
                    buf[pos] = c;
                    len += 1;
                    pos += 1;
                    // Write from inserted char to end of line
                    syscall::write(1, &buf[pos - 1..len]);
                    // Move cursor back to pos
                    let back = len - pos;
                    for _ in 0..back {
                        syscall::write(1, b"\x08");
                    }
                }
            }
            _ => {} // Ignore other control chars
        }
    }
}

/// Replace the current editing line with a history entry.
fn replace_line(buf: &mut [u8], pos: &mut usize, len: &mut usize, hist: &History) {
    // Erase current line on screen
    // Move cursor to start
    while *pos > 0 {
        syscall::write(1, b"\x08");
        *pos -= 1;
    }
    // Overwrite with spaces
    for _ in 0..*len {
        syscall::write(1, b" ");
    }
    // Move back to start
    for _ in 0..*len {
        syscall::write(1, b"\x08");
    }

    if hist.browse >= hist.total() {
        // Past the end: empty line
        *len = 0;
        *pos = 0;
    } else if let Some((line, line_len)) = hist.get(hist.browse) {
        buf[..line_len].copy_from_slice(&line[..line_len]);
        buf[line_len] = 0;
        *len = line_len;
        *pos = line_len;
        syscall::write(1, &buf[..*len]);
    }
}

/// Parse and execute a command line (in the child process).
fn run_cmd(line: &[u8]) {
    let len = line.iter().position(|&b| b == 0).unwrap_or(line.len());
    let line = &line[..len];

    // Check for pipe
    if let Some(pipe_pos) = line.iter().position(|&b| b == b'|') {
        run_pipe(line, pipe_pos);
        return;
    }

    // Parse redirections and build argv
    let mut argv_bufs: [[u8; 64]; MAXARGS] = [[0; 64]; MAXARGS];
    let mut argc = 0usize;
    let mut input_file: Option<[u8; 64]> = None;
    let mut output_file: Option<[u8; 64]> = None;

    let mut i = 0;
    while i < len {
        // Skip whitespace
        while i < len && line[i] == b' ' { i += 1; }
        if i >= len { break; }

        if line[i] == b'<' {
            // Input redirection
            i += 1;
            while i < len && line[i] == b' ' { i += 1; }
            let start = i;
            while i < len && line[i] != b' ' && line[i] != b'&' { i += 1; }
            let mut f = [0u8; 64];
            let flen = (i - start).min(63);
            f[..flen].copy_from_slice(&line[start..start + flen]);
            input_file = Some(f);
        } else if line[i] == b'>' {
            // Output redirection
            i += 1;
            while i < len && line[i] == b' ' { i += 1; }
            let start = i;
            while i < len && line[i] != b' ' && line[i] != b'&' { i += 1; }
            let mut f = [0u8; 64];
            let flen = (i - start).min(63);
            f[..flen].copy_from_slice(&line[start..start + flen]);
            output_file = Some(f);
        } else if line[i] == b'&' {
            i += 1; // skip ampersand
        } else {
            // Regular argument
            let start = i;
            while i < len && line[i] != b' ' && line[i] != b'<' && line[i] != b'>' && line[i] != b'|' && line[i] != b'&' {
                i += 1;
            }
            if argc < MAXARGS {
                let alen = (i - start).min(63);
                argv_bufs[argc][..alen].copy_from_slice(&line[start..start + alen]);
                argc += 1;
            }
        }
    }

    if argc == 0 { return; }

    // Set up redirections
    if let Some(f) = input_file {
        syscall::close(0);
        let fd = syscall::open(f.as_ptr(), syscall::O_RDONLY);
        if fd < 0 {
            println!("sh: cannot open {}", cstr(&f));
            syscall::exit(1);
        }
    }
    if let Some(f) = output_file {
        syscall::close(1);
        let fd = syscall::open(f.as_ptr(), syscall::O_WRONLY | syscall::O_CREATE);
        if fd < 0 {
            println!("sh: cannot open {}", cstr(&f));
            syscall::exit(1);
        }
    }

    // Build argv pointer array
    let mut argv_ptrs: [*const u8; MAXARGS + 1] = [core::ptr::null(); MAXARGS + 1];
    for j in 0..argc {
        argv_ptrs[j] = argv_bufs[j].as_ptr();
    }

    syscall::exec(argv_bufs[0].as_ptr(), argv_ptrs.as_ptr());
    println!("sh: exec {} failed", cstr(&argv_bufs[0]));
    syscall::exit(1);
}

/// Execute a pipe: left | right
fn run_pipe(line: &[u8], pipe_pos: usize) {
    let mut pipefd = [0i32; 2];
    syscall::pipe(&mut pipefd);

    let pid1 = syscall::fork();
    if pid1 == 0 {
        // Left side: stdout -> pipe write end
        syscall::close(1);
        syscall::dup(pipefd[1]);
        syscall::close(pipefd[0]);
        syscall::close(pipefd[1]);
        run_cmd(&line[..pipe_pos]);
        syscall::exit(0);
    }

    let pid2 = syscall::fork();
    if pid2 == 0 {
        // Right side: stdin -> pipe read end
        syscall::close(0);
        syscall::dup(pipefd[0]);
        syscall::close(pipefd[0]);
        syscall::close(pipefd[1]);
        let right = &line[pipe_pos + 1..];
        run_cmd(right);
        syscall::exit(0);
    }

    syscall::close(pipefd[0]);
    syscall::close(pipefd[1]);
    let mut s: i32 = 0;
    syscall::wait(&mut s as *mut i32);
    syscall::wait(&mut s as *mut i32);
}

fn has_ampersand(line: &[u8]) -> bool {
    let len = line.iter().position(|&b| b == 0).unwrap_or(line.len());
    line[..len].contains(&b'&')
}

fn starts_with(line: &[u8], prefix: &[u8]) -> bool {
    if line.len() < prefix.len() { return false; }
    &line[..prefix.len()] == prefix
}

fn cstr(buf: &[u8]) -> &str {
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    unsafe { core::str::from_utf8_unchecked(&buf[..len]) }
}
