use core::arch::asm;

// Syscall numbers
pub const SYS_HALT: u32 = 0;
pub const SYS_EXIT: u32 = 1;
pub const SYS_EXEC: u32 = 2;
pub const SYS_WAIT: u32 = 3;
pub const SYS_CREATE: u32 = 4;
pub const SYS_REMOVE: u32 = 5;
pub const SYS_OPEN: u32 = 6;
pub const SYS_FILESIZE: u32 = 7;
pub const SYS_READ: u32 = 8;
pub const SYS_WRITE: u32 = 9;
pub const SYS_SEEK: u32 = 10;
pub const SYS_TELL: u32 = 11;
pub const SYS_CLOSE: u32 = 12;
pub const SYS_MMAP: u32 = 13;
pub const SYS_MUNMAP: u32 = 14;

pub fn syscall0(num: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {num}",
            "int 0x30",
            "add esp, 4",
            num = in(reg) num,
            lateout("eax") ret,
        );
    }
    ret
}

pub fn syscall1(num: u32, arg0: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {a0}",
            "push {num}",
            "int 0x30",
            "add esp, 8",
            num = in(reg) num,
            a0 = in(reg) arg0,
            lateout("eax") ret,
        );
    }
    ret
}

pub fn syscall2(num: u32, arg0: u32, arg1: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {a1}",
            "push {a0}",
            "push {num}",
            "int 0x30",
            "add esp, 12",
            num = in(reg) num,
            a0 = in(reg) arg0,
            a1 = in(reg) arg1,
            lateout("eax") ret,
        );
    }
    ret
}

pub fn syscall3(num: u32, arg0: u32, arg1: u32, arg2: u32) -> i32 {
    let ret: i32;
    unsafe {
        asm!(
            "push {a2}",
            "push {a1}",
            "push {a0}",
            "push {num}",
            "int 0x30",
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

// === Syscall wrappers ===

/// Halt the OS (power off).
pub fn halt() -> ! {
    syscall0(SYS_HALT);
    loop {}
}

/// Exit the current process with the given status code.
pub fn exit(status: i32) -> ! {
    syscall1(SYS_EXIT, status as u32);
    loop {}
}

/// Execute a program. Returns the new process's pid, or -1 on failure.
/// `cmd_line` must be a pointer to a null-terminated C string.
pub fn exec(cmd_line: *const u8) -> i32 {
    syscall1(SYS_EXEC, cmd_line as u32)
}

/// Wait for child process `pid` to exit. Returns its exit status.
pub fn wait(pid: i32) -> i32 {
    syscall1(SYS_WAIT, pid as u32)
}

/// Create a file with the given initial size.
/// `name` must be a null-terminated byte slice (e.g., b"file.txt\0").
pub fn create(name: &[u8], initial_size: u32) -> bool {
    syscall2(SYS_CREATE, name.as_ptr() as u32, initial_size) != 0
}

/// Remove (delete) a file.
pub fn remove(name: &[u8]) -> bool {
    syscall1(SYS_REMOVE, name.as_ptr() as u32) != 0
}

/// Open a file. Returns a file descriptor, or -1 on failure.
pub fn open(name: &[u8]) -> i32 {
    syscall1(SYS_OPEN, name.as_ptr() as u32)
}

/// Return the size of the file identified by `fd`.
pub fn filesize(fd: i32) -> i32 {
    syscall1(SYS_FILESIZE, fd as u32)
}

/// Read `size` bytes from `fd` into `buf`. Returns number of bytes actually read.
pub fn read(fd: i32, buf: &mut [u8]) -> i32 {
    syscall3(SYS_READ, fd as u32, buf.as_mut_ptr() as u32, buf.len() as u32)
}

/// Write `buf` to `fd`. Returns number of bytes actually written.
pub fn write(fd: i32, buf: &[u8]) -> i32 {
    syscall3(SYS_WRITE, fd as u32, buf.as_ptr() as u32, buf.len() as u32)
}

/// Seek to position `pos` in file `fd`.
pub fn seek(fd: i32, pos: u32) {
    syscall2(SYS_SEEK, fd as u32, pos);
}

/// Return the current position in file `fd`.
pub fn tell(fd: i32) -> u32 {
    syscall1(SYS_TELL, fd as u32) as u32
}

/// Close file descriptor `fd`.
pub fn close(fd: i32) {
    syscall1(SYS_CLOSE, fd as u32);
}

/// Map a file into memory. Returns a mapping id, or -1 on failure.
pub fn mmap(fd: i32, addr: usize) -> i32 {
    syscall2(SYS_MMAP, fd as u32, addr as u32)
}

/// Unmap a memory-mapped file region.
pub fn munmap(mapid: i32) {
    syscall1(SYS_MUNMAP, mapid as u32);
}

/// Create a directory.
pub fn mkdir(path: &[u8]) -> bool {
    syscall1(15, path.as_ptr() as u32) != 0
}

/// Check if a file descriptor refers to a directory.
pub fn isdir(fd: i32) -> bool {
    syscall1(17, fd as u32) != 0
}

/// Return the inode number of the file referred to by `fd`.
pub fn inumber(fd: i32) -> i32 {
    syscall1(18, fd as u32)
}
