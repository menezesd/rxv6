//! System call dispatch for rxv6.
//!
//! UNIX v6/v7/BSD-style syscalls via int 0x80 (DPL=3).
//! Reads syscall number and arguments from the user stack (ESP).

extern crate alloc;

use alloc::string::String;
use crate::arch::idt::{self, IntrFrame, IntrLevel};
use crate::mem::vaddr::{PHYS_BASE, PGSIZE};
use super::process::FdKind;

// Syscall numbers (must match user/rxv6-user/src/syscall.rs)
const SYS_FORK: u32 = 1;
const SYS_EXIT: u32 = 2;
const SYS_WAIT: u32 = 3;
const SYS_PIPE: u32 = 4;
const SYS_READ: u32 = 5;
const SYS_WRITE: u32 = 6;
const SYS_CLOSE: u32 = 7;
const SYS_KILL: u32 = 8;
const SYS_EXEC: u32 = 9;
const SYS_OPEN: u32 = 10;
const SYS_MKNOD: u32 = 11;
const SYS_UNLINK: u32 = 12;
const SYS_FSTAT: u32 = 13;
const SYS_LINK: u32 = 14;
const SYS_MKDIR: u32 = 15;
const SYS_CHDIR: u32 = 16;
const SYS_DUP: u32 = 17;
const SYS_GETPID: u32 = 18;
const SYS_SBRK: u32 = 19;
const SYS_SLEEP: u32 = 20;
const SYS_UPTIME: u32 = 21;

const O_CREATE: i32 = 0x200;

/// Register the syscall interrupt handler (int 0x80, DPL=3).
pub fn init() {
    idt::register_int(0x80, 3, IntrLevel::On, syscall_handler, "syscall");
}

// ---- Validation helpers ----------------------------------------------------

fn is_valid_user_ptr(addr: usize, size: usize) -> bool {
    if !(PGSIZE..PHYS_BASE).contains(&addr) || addr.wrapping_add(size) > PHYS_BASE {
        return false;
    }
    let t = crate::thread::running_thread();
    let pd = unsafe { (*t).pagedir };
    if pd.is_null() { return true; }
    let tid = unsafe { (*t).tid };
    let mut page = addr & !(PGSIZE - 1);
    let end = addr + size;
    while page < end {
        if crate::userprog::pagedir::get_page(pd, page).is_null() {
            let spt = super::process::get_spt(tid);
            if !spt.is_null() {
                let spt_ref = unsafe { &*spt };
                if spt_ref.find(page).is_some() {
                    demand_load_page(pd, page, tid);
                    page += PGSIZE;
                    continue;
                }
            }
            let stack_limit = PHYS_BASE - 8 * 1024 * 1024;
            if page >= stack_limit && page < PHYS_BASE { return true; }
            return false;
        }
        page += PGSIZE;
    }
    true
}

fn demand_load_page(pd: *mut u32, page: usize, tid: i32) {
    let spt = super::process::get_spt(tid);
    if spt.is_null() { return; }
    let spt_ref = unsafe { &mut *spt };
    if let Some(entry) = spt_ref.find_mut(page) {
        crate::vm::page::load_page(entry, pd, page, tid);
    }
}

fn check_args(args: *const u32, count: usize) {
    let end = unsafe { args.add(count + 1) } as usize;
    if !is_valid_user_ptr(args as usize, end - args as usize) {
        exit_process(-1);
    }
}

fn exit_process(status: i32) -> ! {
    let name = crate::thread::current_name();
    crate::kprintln!("{}: exit({})", name, status);
    super::process::exit_with_status(status);
    crate::thread::exit();
}

fn validate_user_buffer(addr: usize, size: usize) {
    if size > 0 && !is_valid_user_ptr(addr, size) {
        exit_process(-1);
    }
}

fn read_user_string(addr: usize) -> String {
    let mut s = String::new();
    let mut ptr = addr;
    loop {
        if !is_valid_user_ptr(ptr, 1) { exit_process(-1); }
        let byte = unsafe { *(ptr as *const u8) };
        if byte == 0 { break; }
        s.push(byte as char);
        ptr += 1;
    }
    s
}

fn pin_user_pages(tid: i32, addr: usize, size: usize) {
    let mut page = addr & !(PGSIZE - 1);
    let end = addr + size;
    while page < end { crate::vm::frame::pin_upage(tid, page); page += PGSIZE; }
}

fn unpin_user_pages(tid: i32, addr: usize, size: usize) {
    let mut page = addr & !(PGSIZE - 1);
    let end = addr + size;
    while page < end { crate::vm::frame::unpin_upage(tid, page); page += PGSIZE; }
}

// ---- I/O helpers -----------------------------------------------------------

fn sys_write(fd: i32, buf_addr: usize, size: usize) -> i32 {
    validate_user_buffer(buf_addr, size);
    if fd < 0 { return -1; }

    let fd_table = super::process::get_fd_table();
    match fd_table.get_kind(fd) {
        Some(FdKind::Console) => {
            let buf = unsafe { core::slice::from_raw_parts(buf_addr as *const u8, size) };
            if let Ok(s) = core::str::from_utf8(buf) {
                crate::kprint!("{}", s);
            } else {
                for &b in buf { crate::devices::serial::putc(b); }
            }
            size as i32
        }
        Some(FdKind::PipeWrite(pipe_id)) => {
            let pipe_id = *pipe_id;
            if let Some(pipe) = crate::pipe::get_pipe(pipe_id) {
                let buf = unsafe { core::slice::from_raw_parts(buf_addr as *const u8, size) };
                pipe.write(buf)
            } else { -1 }
        }
        Some(FdKind::FileDesc(_)) => {
            match fd_table.get(fd) {
                Some(file) => bounced_write(file, buf_addr, size),
                None => -1,
            }
        }
        _ => -1,
    }
}

fn sys_read(fd: i32, buf_addr: usize, size: usize) -> i32 {
    validate_user_buffer(buf_addr, size);
    if fd < 0 { return -1; }

    let fd_table = super::process::get_fd_table();
    match fd_table.get_kind(fd) {
        Some(FdKind::Console) => {
            // Console read = keyboard input
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_addr as *mut u8, size) };
            for b in buf.iter_mut() { *b = crate::devices::input::getc(); }
            size as i32
        }
        Some(FdKind::PipeRead(pipe_id)) => {
            let pipe_id = *pipe_id;
            if let Some(pipe) = crate::pipe::get_pipe(pipe_id) {
                let buf = unsafe { core::slice::from_raw_parts_mut(buf_addr as *mut u8, size) };
                pipe.read(buf)
            } else { -1 }
        }
        Some(FdKind::FileDesc(_)) => {
            match fd_table.get(fd) {
                Some(file) => bounced_read(file, buf_addr, size),
                None => -1,
            }
        }
        _ => -1,
    }
}

fn bounced_read(file: &mut crate::filesys::file::File, buf_addr: usize, size: usize) -> i32 {
    use crate::mem::palloc::{self, PallocFlags};
    let tid = unsafe { (*crate::thread::running_thread()).tid };
    let bounce = palloc::get_page(PallocFlags::ZERO);
    if bounce.is_null() { return -1; }
    pin_user_pages(tid, buf_addr, size);
    let mut total = 0i32;
    let mut offset = 0usize;
    while offset < size {
        let chunk = (size - offset).min(PGSIZE);
        let bounce_slice = unsafe { core::slice::from_raw_parts_mut(bounce, chunk) };
        let n = file.read(bounce_slice, chunk as i32);
        if n <= 0 { break; }
        unsafe { core::ptr::copy_nonoverlapping(bounce, (buf_addr + offset) as *mut u8, n as usize); }
        total += n;
        offset += n as usize;
        if (n as usize) < chunk { break; }
    }
    unpin_user_pages(tid, buf_addr, size);
    palloc::free_page(bounce);
    total
}

fn bounced_write(file: &mut crate::filesys::file::File, buf_addr: usize, size: usize) -> i32 {
    use crate::mem::palloc::{self, PallocFlags};
    let tid = unsafe { (*crate::thread::running_thread()).tid };
    let bounce = palloc::get_page(PallocFlags::ZERO);
    if bounce.is_null() { return -1; }
    pin_user_pages(tid, buf_addr, size);
    let mut total = 0i32;
    let mut offset = 0usize;
    while offset < size {
        let chunk = (size - offset).min(PGSIZE);
        unsafe { core::ptr::copy_nonoverlapping((buf_addr + offset) as *const u8, bounce, chunk); }
        let bounce_slice = unsafe { core::slice::from_raw_parts(bounce, chunk) };
        let n = file.write(bounce_slice, chunk as i32);
        if n <= 0 { break; }
        total += n;
        offset += n as usize;
        if (n as usize) < chunk { break; }
    }
    unpin_user_pages(tid, buf_addr, size);
    palloc::free_page(bounce);
    total
}

// ---- Stat structure (must match user-space) --------------------------------

#[repr(C)]
struct Stat {
    file_type: u16,  // 0=none, 1=dir, 2=file, 3=dev
    dev: u16,
    ino: u32,
    nlink: u16,
    size: u32,
}

// ---- Syscall handler -------------------------------------------------------

fn syscall_handler(frame: &mut IntrFrame) {
    let esp = frame.esp as usize;
    if !is_valid_user_ptr(esp, 4) { exit_process(-1); }

    let args = esp as *const u32;
    let syscall_num = unsafe { *args };

    match syscall_num {
        SYS_FORK => {
            let child_tid = super::process::fork(frame);
            frame.eax = child_tid as u32;
        }

        SYS_EXIT => {
            check_args(args, 1);
            let status = unsafe { *args.add(1) } as i32;
            exit_process(status);
        }

        SYS_WAIT => {
            check_args(args, 1);
            let status_ptr = unsafe { *args.add(1) } as usize;
            let tid = unsafe { (*crate::thread::running_thread()).tid };
            let (child_pid, status) = super::process::wait_any_child(tid);
            if status_ptr != 0 && is_valid_user_ptr(status_ptr, 4) {
                unsafe { *(status_ptr as *mut i32) = status; }
            }
            frame.eax = child_pid as u32;
        }

        SYS_PIPE => {
            check_args(args, 1);
            let fd_ptr = unsafe { *args.add(1) } as usize;
            validate_user_buffer(fd_ptr, 8); // two i32s

            let pipe_id = crate::pipe::create_pipe();
            let fd_table = super::process::get_fd_table();
            let rfd = fd_table.open_pipe_read(pipe_id);
            let wfd = fd_table.open_pipe_write(pipe_id);

            unsafe {
                *(fd_ptr as *mut i32) = rfd;
                *((fd_ptr + 4) as *mut i32) = wfd;
            }
            frame.eax = 0;
        }

        SYS_READ => {
            check_args(args, 3);
            let fd = unsafe { *args.add(1) } as i32;
            let buf = unsafe { *args.add(2) } as usize;
            let size = unsafe { *args.add(3) } as usize;
            if size > PHYS_BASE { exit_process(-1); }
            frame.eax = sys_read(fd, buf, size) as u32;
        }

        SYS_WRITE => {
            check_args(args, 3);
            let fd = unsafe { *args.add(1) } as i32;
            let buf = unsafe { *args.add(2) } as usize;
            let size = unsafe { *args.add(3) } as usize;
            if size > PHYS_BASE { exit_process(-1); }
            frame.eax = sys_write(fd, buf, size) as u32;
        }

        SYS_CLOSE => {
            check_args(args, 1);
            let fd = unsafe { *args.add(1) } as i32;
            let fd_table = super::process::get_fd_table();
            frame.eax = if fd_table.close(fd) { 0u32 } else { (-1i32) as u32 };
        }

        SYS_KILL => {
            check_args(args, 1);
            let pid = unsafe { *args.add(1) } as i32;
            // Mark the target thread for termination.
            // In xv6, kill just sets p->killed; the process exits on next trap return.
            // Simplified: we just check if the thread exists.
            let found = crate::thread::kill_thread(pid);
            frame.eax = if found { 0u32 } else { (-1i32) as u32 };
        }

        SYS_EXEC => {
            check_args(args, 2);
            let path_ptr = unsafe { *args.add(1) } as usize;
            let _argv_ptr = unsafe { *args.add(2) } as usize;
            if !is_valid_user_ptr(path_ptr, 1) { exit_process(-1); }
            let path = read_user_string(path_ptr);
            // TODO: full exec (replace current process image)
            // For now: spawn child and return its pid
            let tid = super::process::execute(&path);
            frame.eax = tid as u32;
        }

        SYS_OPEN => {
            check_args(args, 2);
            let path_ptr = unsafe { *args.add(1) } as usize;
            let flags = unsafe { *args.add(2) } as i32;
            if !is_valid_user_ptr(path_ptr, 1) { exit_process(-1); }
            let path = read_user_string(path_ptr);

            if (flags & O_CREATE) != 0 {
                if crate::filesys::filesys::open(&path).is_none() {
                    crate::filesys::filesys::create(&path, 0);
                }
            }

            match crate::filesys::filesys::open(&path) {
                Some(file) => {
                    let fd_table = super::process::get_fd_table();
                    frame.eax = fd_table.open(file) as u32;
                }
                None => frame.eax = (-1i32) as u32,
            }
        }

        SYS_MKNOD => {
            // Device special files not supported yet
            frame.eax = (-1i32) as u32;
        }

        SYS_UNLINK => {
            check_args(args, 1);
            let path_ptr = unsafe { *args.add(1) } as usize;
            if !is_valid_user_ptr(path_ptr, 1) { exit_process(-1); }
            let path = read_user_string(path_ptr);
            frame.eax = if crate::filesys::filesys::remove(&path) { 0u32 } else { (-1i32) as u32 };
        }

        SYS_FSTAT => {
            check_args(args, 2);
            let fd = unsafe { *args.add(1) } as i32;
            let st_ptr = unsafe { *args.add(2) } as usize;
            validate_user_buffer(st_ptr, core::mem::size_of::<Stat>());

            let fd_table = super::process::get_fd_table();
            match fd_table.get(fd) {
                Some(file) => {
                    let sector = file.inode_sector;
                    let is_dir = crate::filesys::inode::is_dir(sector);
                    let length = crate::filesys::inode::length(sector);
                    let stat = Stat {
                        file_type: if is_dir { 1 } else { 2 },
                        dev: 0,
                        ino: sector,
                        nlink: 1,
                        size: length as u32,
                    };
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            &stat as *const Stat as *const u8,
                            st_ptr as *mut u8,
                            core::mem::size_of::<Stat>(),
                        );
                    }
                    frame.eax = 0;
                }
                None => frame.eax = (-1i32) as u32,
            }
        }

        SYS_LINK => {
            check_args(args, 2);
            let old_ptr = unsafe { *args.add(1) } as usize;
            let new_ptr = unsafe { *args.add(2) } as usize;
            if !is_valid_user_ptr(old_ptr, 1) || !is_valid_user_ptr(new_ptr, 1) {
                exit_process(-1);
            }
            let old_path = read_user_string(old_ptr);
            let new_path = read_user_string(new_ptr);
            frame.eax = if crate::filesys::filesys::link(&old_path, &new_path) { 0u32 } else { (-1i32) as u32 };
        }

        SYS_MKDIR => {
            check_args(args, 1);
            let path_ptr = unsafe { *args.add(1) } as usize;
            if !is_valid_user_ptr(path_ptr, 1) { exit_process(-1); }
            let path = read_user_string(path_ptr);
            frame.eax = if crate::filesys::filesys::mkdir(&path) { 0u32 } else { (-1i32) as u32 };
        }

        SYS_CHDIR => {
            check_args(args, 1);
            let path_ptr = unsafe { *args.add(1) } as usize;
            if !is_valid_user_ptr(path_ptr, 1) { exit_process(-1); }
            let path = read_user_string(path_ptr);
            // Look up the directory
            match crate::filesys::filesys::open(&path) {
                Some(file) => {
                    let sector = file.inode_sector;
                    if crate::filesys::inode::is_dir(sector) {
                        let t = crate::thread::running_thread();
                        unsafe { (*t).cwd_sector = sector; }
                        frame.eax = 0;
                    } else {
                        frame.eax = (-1i32) as u32;
                    }
                    file.close_file();
                }
                None => frame.eax = (-1i32) as u32,
            }
        }

        SYS_DUP => {
            check_args(args, 1);
            let fd = unsafe { *args.add(1) } as i32;
            let fd_table = super::process::get_fd_table();
            frame.eax = fd_table.dup(fd) as u32;
        }

        SYS_GETPID => {
            frame.eax = unsafe { (*crate::thread::running_thread()).tid } as u32;
        }

        SYS_SBRK => {
            check_args(args, 1);
            let n = unsafe { *args.add(1) } as i32;
            let t = crate::thread::running_thread();
            let old_brk = unsafe { (*t).brk };

            if old_brk == 0 {
                // brk not initialized — can't sbrk yet
                frame.eax = (-1i32) as u32;
            } else {
                let new_brk = if n >= 0 {
                    old_brk + n as usize
                } else {
                    old_brk.saturating_sub((-n) as usize)
                };

                if new_brk >= PHYS_BASE {
                    frame.eax = (-1i32) as u32;
                } else {
                    // Allocate pages for the new region if growing
                    if new_brk > old_brk {
                        let tid = unsafe { (*t).tid };
                        let pd = unsafe { (*t).pagedir };
                        let mut page = old_brk & !(PGSIZE - 1);
                        if old_brk & (PGSIZE - 1) != 0 { page += PGSIZE; } // skip partial page
                        while page < new_brk {
                            if crate::userprog::pagedir::get_page(pd, page).is_null() {
                                let kpage = crate::vm::frame::alloc_frame(page, tid);
                                if kpage.is_null() {
                                    frame.eax = (-1i32) as u32;
                                    return;
                                }
                                crate::userprog::pagedir::set_page(pd, page, kpage as usize, true);
                            }
                            page += PGSIZE;
                        }
                    }
                    unsafe { (*t).brk = new_brk; }
                    frame.eax = old_brk as u32;
                }
            }
        }

        SYS_SLEEP => {
            check_args(args, 1);
            let ticks = unsafe { *args.add(1) } as u32;
            crate::devices::timer::timer_sleep(ticks as i64);
            frame.eax = 0;
        }

        SYS_UPTIME => {
            frame.eax = crate::devices::timer::ticks() as u32;
        }

        _ => {
            crate::kprintln!("unknown syscall: {}", syscall_num);
            exit_process(-1);
        }
    }
}
