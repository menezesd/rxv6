//! sh - a simple UNIX shell.
//!
//! Supports:
//! - Simple commands: cmd arg1 arg2
//! - Pipes: cmd1 | cmd2
//! - Output redirection: cmd > file
//! - Input redirection: cmd < file
//! - Background: cmd &
//! - Built-in: cd dir

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

const MAXARGS: usize = 16;
const BUFSIZE: usize = 256;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut buf = [0u8; BUFSIZE];

    loop {
        print!("$ ");
        let n = getline(&mut buf);
        if n <= 0 {
            break;
        }
        // Remove trailing newline
        let len = n as usize;
        if len > 0 && buf[len - 1] == b'\n' {
            buf[len - 1] = 0;
        }

        let line = &buf[..len];
        if line.is_empty() || line[0] == 0 {
            continue;
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

        // Fork and execute
        let pid = syscall::fork();
        if pid < 0 {
            println!("sh: fork failed");
            continue;
        }
        if pid == 0 {
            // Child: parse and execute the command
            run_cmd(line);
            syscall::exit(0);
        }
        // Parent: wait for child (unless background)
        let bg = has_ampersand(line);
        if !bg {
            let mut status: i32 = 0;
            syscall::wait(&mut status as *mut i32);
        }
    }
    0
}

/// Read a line from stdin into buf. Returns bytes read.
fn getline(buf: &mut [u8]) -> i32 {
    let mut i = 0usize;
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
