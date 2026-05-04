//! xargs - build and execute command lines from stdin.
//!
//! Usage: xargs [command [initial-args]]
//! Reads lines from stdin, appends each as arguments to command, and executes.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::println;

const MAX_ARGS: usize = 64;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    // Collect base command and initial args
    let mut base_argv: [[u8; 64]; MAX_ARGS] = [[0; 64]; MAX_ARGS];
    let mut base_argc = 0usize;

    if argc > 1 {
        for i in 1..argc as usize {
            let arg = unsafe { rxv6_user::cstr_to_str(*argv.add(i)) };
            let len = arg.len().min(63);
            base_argv[base_argc][..len].copy_from_slice(&arg.as_bytes()[..len]);
            base_argc += 1;
        }
    } else {
        // Default command is "echo"
        base_argv[0][..4].copy_from_slice(b"echo");
        base_argc = 1;
    }

    // Read lines from stdin and execute
    let mut line = [0u8; 256];
    let mut line_len = 0usize;
    let mut buf = [0u8; 128];
    let mut status = 0i32;

    loop {
        let n = syscall::read(0, &mut buf);
        if n <= 0 {
            if line_len > 0 {
                status = run_line(&base_argv, base_argc, &line, line_len);
            }
            break;
        }
        for i in 0..n as usize {
            if buf[i] == b'\n' {
                if line_len > 0 {
                    status = run_line(&base_argv, base_argc, &line, line_len);
                    line_len = 0;
                }
            } else if line_len < line.len() - 1 {
                line[line_len] = buf[i];
                line_len += 1;
            }
        }
    }
    status
}

fn run_line(base: &[[u8; 64]; MAX_ARGS], base_argc: usize, line: &[u8], len: usize) -> i32 {
    // Split line by whitespace into additional args
    let mut all_argv: [[u8; 64]; MAX_ARGS] = [[0; 64]; MAX_ARGS];
    let mut argc = 0usize;

    // Copy base args
    for i in 0..base_argc {
        all_argv[i] = base[i];
        argc += 1;
    }

    // Parse line into words
    let mut i = 0;
    while i < len && argc < MAX_ARGS {
        while i < len && (line[i] == b' ' || line[i] == b'\t') { i += 1; }
        if i >= len { break; }
        let start = i;
        while i < len && line[i] != b' ' && line[i] != b'\t' { i += 1; }
        let wlen = (i - start).min(63);
        all_argv[argc][..wlen].copy_from_slice(&line[start..start + wlen]);
        all_argv[argc][wlen] = 0;
        argc += 1;
    }

    // Fork and exec
    let pid = syscall::fork();
    if pid == 0 {
        let mut ptrs: [*const u8; MAX_ARGS + 1] = [core::ptr::null(); MAX_ARGS + 1];
        for j in 0..argc {
            ptrs[j] = all_argv[j].as_ptr();
        }
        syscall::exec(all_argv[0].as_ptr(), ptrs.as_ptr());
        println!("xargs: exec failed");
        syscall::exit(127);
    }
    let mut status: i32 = 0;
    syscall::wait(&mut status);
    status
}
