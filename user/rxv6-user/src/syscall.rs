//! rxv6 system call interface.
//!
//! UNIX v6/v7/BSD-style syscalls via int 0x80.
//! Arguments passed on the user stack: [syscall_num] [arg0] [arg1] [arg2] ...
//! Return value in eax.

use core::arch::asm;

// Syscall numbers (xv6-style)
pub const SYS_FORK: u32 = 1;
pub const SYS_EXIT: u32 = 2;
pub const SYS_WAIT: u32 = 3;
pub const SYS_PIPE: u32 = 4;
pub const SYS_READ: u32 = 5;
pub const SYS_WRITE: u32 = 6;
pub const SYS_CLOSE: u32 = 7;
pub const SYS_KILL: u32 = 8;
pub const SYS_EXEC: u32 = 9;
pub const SYS_OPEN: u32 = 10;
pub const SYS_MKNOD: u32 = 11;
pub const SYS_UNLINK: u32 = 12;
pub const SYS_FSTAT: u32 = 13;
pub const SYS_LINK: u32 = 14;
pub const SYS_MKDIR: u32 = 15;
pub const SYS_CHDIR: u32 = 16;
pub const SYS_DUP: u32 = 17;
pub const SYS_GETPID: u32 = 18;
pub const SYS_SBRK: u32 = 19;
pub const SYS_SLEEP: u32 = 20;
pub const SYS_UPTIME: u32 = 21;
pub const SYS_IOCTL: u32 = 22;
pub const SYS_SIGNAL: u32 = 23;
pub const SYS_SETPGID: u32 = 24;
pub const SYS_GETPGID: u32 = 25;

// Signal numbers
pub const SIGHUP: u32 = 1;
pub const SIGINT: u32 = 2;
pub const SIGQUIT: u32 = 3;
pub const SIGKILL: u32 = 9;
pub const SIGPIPE: u32 = 13;
pub const SIGTERM: u32 = 15;
pub const SIGCHLD: u32 = 17;

pub const SIG_DFL: u32 = 0;
pub const SIG_IGN: u32 = 1;

fn syscall0(num: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {num}",
            "int 0x80",
            "add esp, 4",
            num = in(reg) num,
            lateout("eax") ret,
        );
    }
    ret
}

fn syscall1(num: u32, arg0: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {a0}",
            "push {num}",
            "int 0x80",
            "add esp, 8",
            num = in(reg) num,
            a0 = in(reg) arg0,
            lateout("eax") ret,
        );
    }
    ret
}

fn syscall2(num: u32, arg0: u32, arg1: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {a1}",
            "push {a0}",
            "push {num}",
            "int 0x80",
            "add esp, 12",
            num = in(reg) num,
            a0 = in(reg) arg0,
            a1 = in(reg) arg1,
            lateout("eax") ret,
        );
    }
    ret
}

fn syscall3(num: u32, arg0: u32, arg1: u32, arg2: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {a2}",
            "push {a1}",
            "push {a0}",
            "push {num}",
            "int 0x80",
            "add esp, 16",
            num = in(reg) num,
            a0 = in(reg) arg0,
            a1 = in(reg) arg1,
            a2 = in(reg) arg2,
            lateout("eax") ret,
        );
    }
    ret
}

// === UNIX syscall wrappers ===

/// Fork the current process. Returns 0 in child, child pid in parent, -1 on error.
pub fn fork() -> i32 {
    syscall0(SYS_FORK)
}

/// Exit the current process with the given status.
pub fn exit(status: i32) -> ! {
    syscall1(SYS_EXIT, status as u32);
    loop {}
}

/// Wait for any child to exit. Stores exit status in *status. Returns child pid.
pub fn wait(status: *mut i32) -> i32 {
    syscall1(SYS_WAIT, status as u32)
}

/// Create a pipe. pipefd[0] is read end, pipefd[1] is write end.
pub fn pipe(pipefd: &mut [i32; 2]) -> i32 {
    syscall1(SYS_PIPE, pipefd.as_mut_ptr() as u32)
}

/// Read up to buf.len() bytes from fd into buf. Returns bytes read.
pub fn read(fd: i32, buf: &mut [u8]) -> i32 {
    syscall3(SYS_READ, fd as u32, buf.as_mut_ptr() as u32, buf.len() as u32)
}

/// Write buf to fd. Returns bytes written.
pub fn write(fd: i32, buf: &[u8]) -> i32 {
    syscall3(SYS_WRITE, fd as u32, buf.as_ptr() as u32, buf.len() as u32)
}

/// Close file descriptor.
pub fn close(fd: i32) -> i32 {
    syscall1(SYS_CLOSE, fd as u32)
}

/// Send a signal to a process or process group.
/// pid > 0: send to that process
/// pid == 0: send to own process group
/// pid < -1: send to process group |pid|
/// sig == 0: use SIGKILL for backwards compatibility
pub fn kill(pid: i32, sig: u32) -> i32 {
    syscall2(SYS_KILL, pid as u32, sig)
}

/// Set signal disposition. Returns previous disposition.
pub fn signal(sig: u32, handler: u32) -> i32 {
    syscall2(SYS_SIGNAL, sig, handler)
}

/// Set process group ID.
pub fn setpgid(pid: i32, pgid: i32) -> i32 {
    syscall2(SYS_SETPGID, pid as u32, pgid as u32)
}

/// Get process group ID.
pub fn getpgid(pid: i32) -> i32 {
    syscall1(SYS_GETPGID, pid as u32)
}

/// Replace current process with new program. argv is null-terminated array.
pub fn exec(path: *const u8, argv: *const *const u8) -> i32 {
    syscall2(SYS_EXEC, path as u32, argv as u32)
}

/// Open a file. flags: O_RDONLY=0, O_WRONLY=1, O_RDWR=2, O_CREATE=0x200.
pub fn open(path: *const u8, flags: i32) -> i32 {
    syscall2(SYS_OPEN, path as u32, flags as u32)
}

/// Create a device special file.
pub fn mknod(path: *const u8, major: i16, minor: i16) -> i32 {
    syscall3(SYS_MKNOD, path as u32, major as u32, minor as u32)
}

/// Remove a file or empty directory.
pub fn unlink(path: *const u8) -> i32 {
    syscall1(SYS_UNLINK, path as u32)
}

/// Get file status.
pub fn fstat(fd: i32, st: *mut Stat) -> i32 {
    syscall2(SYS_FSTAT, fd as u32, st as u32)
}

/// Create a hard link: new name for existing file.
pub fn link(old: *const u8, new: *const u8) -> i32 {
    syscall2(SYS_LINK, old as u32, new as u32)
}

/// Create a directory.
pub fn mkdir(path: *const u8) -> i32 {
    syscall1(SYS_MKDIR, path as u32)
}

/// Change working directory.
pub fn chdir(path: *const u8) -> i32 {
    syscall1(SYS_CHDIR, path as u32)
}

/// Duplicate a file descriptor. Returns new fd.
pub fn dup(fd: i32) -> i32 {
    syscall1(SYS_DUP, fd as u32)
}

/// Get current process ID.
pub fn getpid() -> i32 {
    syscall0(SYS_GETPID)
}

/// Grow/shrink process data segment by n bytes. Returns old break, or -1.
pub fn sbrk(n: i32) -> i32 {
    syscall1(SYS_SBRK, n as u32)
}

/// Sleep for n timer ticks.
pub fn sleep(n: i32) -> i32 {
    syscall1(SYS_SLEEP, n as u32)
}

/// Return system uptime in timer ticks.
pub fn uptime() -> i32 {
    syscall0(SYS_UPTIME)
}

/// ioctl on a file descriptor.
pub fn ioctl(fd: i32, request: u32, arg: u32) -> i32 {
    syscall3(SYS_IOCTL, fd as u32, request, arg)
}

/// ioctl request: set terminal raw mode. arg: 0=cooked, 1=raw.
/// Returns previous mode.
pub const TIOCRAW: u32 = 0x5401;

/// Set terminal to raw mode (no echo, no line editing, immediate delivery).
pub fn set_raw_mode(raw: bool) -> bool {
    ioctl(0, TIOCRAW, raw as u32) != 0
}

/// Poll: returns true if console input is available for reading.
pub const TIOCPOLL: u32 = 0x5402;

pub fn input_ready() -> bool {
    ioctl(0, TIOCPOLL, 0) != 0
}

// === File types and stat ===

#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    None = 0,
    Dir = 1,
    File = 2,
    Dev = 3,
}

/// File status returned by fstat().
#[repr(C)]
pub struct Stat {
    pub file_type: FileType,
    pub dev: u16,
    pub ino: u32,
    pub nlink: u16,
    pub size: u32,
}

// Open flags
pub const O_RDONLY: i32 = 0;
pub const O_WRONLY: i32 = 1;
pub const O_RDWR: i32 = 2;
pub const O_CREATE: i32 = 0x200;
