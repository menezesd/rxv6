//! System call dispatch for RustOS.
//!
//! Registers interrupt 0x30 (DPL=3 so user code can invoke it).
//! Reads the syscall number and arguments from the user stack (ESP).

extern crate alloc;

use alloc::string::String;
use crate::arch::idt::{self, IntrFrame, IntrLevel};
use crate::mem::vaddr::{PHYS_BASE, PGSIZE};

// Syscall numbers (Pintos convention)
const SYS_HALT: u32 = 0;
const SYS_EXIT: u32 = 1;
const SYS_EXEC: u32 = 2;
const SYS_WAIT: u32 = 3;
const SYS_CREATE: u32 = 4;
const SYS_REMOVE: u32 = 5;
const SYS_OPEN: u32 = 6;
const SYS_FILESIZE: u32 = 7;
const SYS_READ: u32 = 8;
const SYS_WRITE: u32 = 9;
const SYS_SEEK: u32 = 10;
const SYS_TELL: u32 = 11;
const SYS_CLOSE: u32 = 12;
const SYS_MMAP: u32 = 13;
const SYS_MUNMAP: u32 = 14;
const SYS_MKDIR: u32 = 15;
const SYS_READDIR: u32 = 16;
const SYS_ISDIR: u32 = 17;
const SYS_INUMBER: u32 = 18;

/// Register the syscall interrupt handler (int 0x30, DPL=3).
pub fn init() {
    idt::register_int(0x30, 3, IntrLevel::On, syscall_handler, "syscall");
}

/// Validate that a user-space pointer is in valid range.
/// Checks bounds AND page mapping (like Pintos valid_user_address).
fn is_valid_user_ptr(addr: usize, size: usize) -> bool {
    if !(PGSIZE..PHYS_BASE).contains(&addr) || addr.wrapping_add(size) > PHYS_BASE {
        return false;
    }
    // Check each page the range spans is mapped, demand-pageable, or in stack region.
    let t = crate::thread::running_thread();
    let pd = unsafe { (*t).pagedir };
    if pd.is_null() {
        return true; // kernel thread, no page directory
    }
    let tid = unsafe { (*t).tid };
    let mut page = addr & !(PGSIZE - 1);
    let end = addr + size;
    while page < end {
        if crate::userprog::pagedir::get_page(pd, page).is_null() {
            // Not mapped — check if it's a demand-paged address (has SPT entry).
            let spt = super::process::get_spt(tid);
            if !spt.is_null() {
                let spt_ref = unsafe { &*spt };
                if spt_ref.find(page).is_some() {
                    // Valid demand-paged address. Pre-fault it so kernel can access it.
                    demand_load_page(pd, page, tid);
                    page += PGSIZE;
                    continue;
                }
            }
            // Check if it's in the stack growth region.
            let stack_limit = PHYS_BASE - 8 * 1024 * 1024;
            if page >= stack_limit && page < PHYS_BASE {
                return true;
            }
            return false;
        }
        page += PGSIZE;
    }
    true
}

/// Pre-fault a demand-paged page so the kernel can safely access it.
/// Called from syscall validation when a user pointer refers to an unmapped
/// but valid (SPT-tracked) page.
fn demand_load_page(pd: *mut u32, page: usize, tid: i32) {
    let spt = super::process::get_spt(tid);
    if spt.is_null() { return; }
    let spt_ref = unsafe { &mut *spt };
    if let Some(entry) = spt_ref.find_mut(page) {
        crate::vm::page::load_page(entry, pd, page, tid);
    }
}

/// Validate that we can read `count` additional u32 arguments starting at args.
fn check_args(args: *const u32, count: usize) {
    let end = unsafe { args.add(count + 1) } as usize;
    if !is_valid_user_ptr(args as usize, end - args as usize) {
        exit_process(-1);
    }
}

/// Terminate the current process with the given exit status.
fn exit_process(status: i32) -> ! {
    let name = crate::thread::current_name();
    crate::kprintln!("{}: exit({})", name, status);
    super::process::exit_with_status(status);
    crate::thread::exit();
}

/// Halt the machine.
fn halt() -> ! {
    crate::devices::shutdown::power_off();
}

/// Read a null-terminated string from user space.
fn read_user_string(addr: usize) -> String {
    let mut s = String::new();
    let mut ptr = addr;
    loop {
        if !is_valid_user_ptr(ptr, 1) {
            exit_process(-1);
        }
        let byte = unsafe { *(ptr as *const u8) };
        if byte == 0 {
            break;
        }
        s.push(byte as char);
        ptr += 1;
    }
    s
}

/// Validate a user buffer of given size.
fn validate_user_buffer(addr: usize, size: usize) {
    if size > 0 && !is_valid_user_ptr(addr, size) {
        exit_process(-1);
    }
}

/// Write to a file descriptor using a kernel bounce buffer.
/// The bounce buffer prevents issues if user pages are evicted during I/O.
fn sys_write(fd: i32, buf_addr: usize, size: usize) -> i32 {
    validate_user_buffer(buf_addr, size);

    if fd == 1 {
        // Write to console (stdout) - safe to access user memory directly
        // since console I/O doesn't trigger disk operations
        let buf = unsafe { core::slice::from_raw_parts(buf_addr as *const u8, size) };
        if let Ok(s) = core::str::from_utf8(buf) {
            crate::kprint!("{}", s);
        } else {
            for &b in buf {
                crate::devices::serial::putc(b);
            }
        }
        return size as i32;
    }

    if fd < 2 { return -1; }

    let fd_table = super::process::get_fd_table();
    match fd_table.get(fd) {
        Some(file) => {
            // Use bounce buffer for file I/O (prevents eviction issues)
            bounced_write(file, buf_addr, size)
        }
        None => -1,
    }
}

/// Read from a file descriptor using a kernel bounce buffer.
fn sys_read(fd: i32, buf_addr: usize, size: usize) -> i32 {
    validate_user_buffer(buf_addr, size);

    if fd == 0 {
        let buf = unsafe { core::slice::from_raw_parts_mut(buf_addr as *mut u8, size) };
        for b in buf.iter_mut().take(size) {
            *b = crate::devices::input::getc();
        }
        return size as i32;
    }
    if fd == 1 { return -1; }

    let fd_table = super::process::get_fd_table();
    match fd_table.get(fd) {
        Some(file) => {
            // Use bounce buffer for file I/O
            bounced_read(file, buf_addr, size)
        }
        None => -1,
    }
}

/// Pin all user pages in [addr, addr+size) to prevent eviction during syscall I/O.
fn pin_user_pages(tid: i32, addr: usize, size: usize) {
    let mut page = addr & !(PGSIZE - 1);
    let end = addr + size;
    while page < end {
        crate::vm::frame::pin_upage(tid, page);
        page += PGSIZE;
    }
}

/// Unpin all user pages in [addr, addr+size).
fn unpin_user_pages(tid: i32, addr: usize, size: usize) {
    let mut page = addr & !(PGSIZE - 1);
    let end = addr + size;
    while page < end {
        crate::vm::frame::unpin_upage(tid, page);
        page += PGSIZE;
    }
}

/// Read from file into user buffer via kernel bounce buffer (like Pintos bounced_io).
fn bounced_read(file: &mut crate::filesys::file::File, buf_addr: usize, size: usize) -> i32 {
    use crate::mem::palloc::{self, PallocFlags};
    let tid = unsafe { (*crate::thread::running_thread()).tid };
    let bounce = palloc::get_page(PallocFlags::ZERO);
    if bounce.is_null() { return -1; }

    // Pin user pages to prevent eviction during disk I/O.
    pin_user_pages(tid, buf_addr, size);

    let mut total = 0i32;
    let mut offset = 0usize;
    while offset < size {
        let chunk = (size - offset).min(PGSIZE);
        let bounce_slice = unsafe { core::slice::from_raw_parts_mut(bounce, chunk) };
        let n = file.read(bounce_slice, chunk as i32);
        if n <= 0 { break; }
        // Copy from bounce to user buffer (safe — user pages are pinned)
        unsafe {
            core::ptr::copy_nonoverlapping(bounce, (buf_addr + offset) as *mut u8, n as usize);
        }
        total += n;
        offset += n as usize;
        if (n as usize) < chunk { break; }
    }

    unpin_user_pages(tid, buf_addr, size);
    palloc::free_page(bounce);
    total
}

/// Write from user buffer to file via kernel bounce buffer.
fn bounced_write(file: &mut crate::filesys::file::File, buf_addr: usize, size: usize) -> i32 {
    use crate::mem::palloc::{self, PallocFlags};
    let tid = unsafe { (*crate::thread::running_thread()).tid };
    let bounce = palloc::get_page(PallocFlags::ZERO);
    if bounce.is_null() { return -1; }

    // Pin user pages to prevent eviction during disk I/O.
    pin_user_pages(tid, buf_addr, size);

    let mut total = 0i32;
    let mut offset = 0usize;
    while offset < size {
        let chunk = (size - offset).min(PGSIZE);
        // Copy from user buffer to bounce (safe — user pages are pinned)
        unsafe {
            core::ptr::copy_nonoverlapping((buf_addr + offset) as *const u8, bounce, chunk);
        }
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

/// Map a file into the user's virtual address space.
fn sys_mmap(fd: i32, addr: usize) -> i32 {
    // Validate arguments
    if fd < 2 || addr == 0 || !addr.is_multiple_of(PGSIZE) || addr >= PHYS_BASE {
        return -1;
    }

    let fd_table = super::process::get_fd_table();
    let (inode_sector, length) = match fd_table.get(fd) {
        Some(file) => {
            let len = file.length() as usize;
            (file.inode_sector, len)
        }
        None => return -1,
    };

    if length == 0 {
        return -1;
    }

    let t = crate::thread::running_thread();
    let pd = unsafe { (*t).pagedir };
    let tid = unsafe { (*t).tid };

    // Check no overlap with existing page mappings
    let end = addr + length;
    let mut page = addr;
    while page < end {
        if !super::pagedir::get_page(pd, page).is_null() {
            return -1;
        }
        page += PGSIZE;
    }

    // Also check overlap with other mmaps
    if super::process::mmap_overlaps(tid, addr, length) {
        return -1;
    }

    // Reopen the file's inode so closing the fd doesn't affect the mapping
    crate::filesys::inode::open(inode_sector);

    // Eagerly load all pages
    let mut offset = 0usize;
    let mut cur_addr = addr;
    while offset < length {
        let kpage = crate::vm::frame::alloc_frame(cur_addr, tid);
        if kpage.is_null() {
            // Clean up already-mapped pages on failure
            let mut cleanup_addr = addr;
            while cleanup_addr < cur_addr {
                let kp = super::pagedir::get_page(pd, cleanup_addr);
                if !kp.is_null() {
                    super::pagedir::clear_page(pd, cleanup_addr);
                    crate::vm::frame::free_frame((kp as usize & !(PGSIZE - 1)) as *mut u8);
                }
                cleanup_addr += PGSIZE;
            }
            crate::filesys::inode::close(inode_sector);
            return -1;
        }

        let read_bytes = (length - offset).min(PGSIZE);
        let buf = unsafe { core::slice::from_raw_parts_mut(kpage, read_bytes) };
        crate::filesys::inode::read_at(inode_sector, buf, read_bytes as i32, offset as i32);

        // Zero rest of page
        if read_bytes < PGSIZE {
            unsafe {
                core::ptr::write_bytes(kpage.add(read_bytes), 0, PGSIZE - read_bytes);
            }
        }

        super::pagedir::set_page(pd, cur_addr, kpage as usize, true);
        offset += PGSIZE;
        cur_addr += PGSIZE;
    }

    // Record mapping
    let mapid = super::process::allocate_mapid();
    super::process::add_mmap(
        tid,
        super::process::MmapEntry {
            mapid,
            fd,
            inode_sector,
            uaddr: addr,
            length,
        },
    );
    mapid
}

/// Unmap a memory-mapped file region.
fn sys_munmap(mapid: i32) {
    let tid = unsafe { (*crate::thread::running_thread()).tid };
    let entry = match super::process::remove_mmap(tid, mapid) {
        Some(e) => e,
        None => return,
    };

    super::process::munmap_entry(&entry);
    // Close the inode we reopened during mmap
    crate::filesys::inode::close(entry.inode_sector);
}

/// Syscall interrupt handler. Dispatches based on syscall number at ESP.
fn syscall_handler(frame: &mut IntrFrame) {
    let esp = frame.esp as usize;
    if !is_valid_user_ptr(esp, 4) {
        exit_process(-1);
    }

    let args = esp as *const u32;
    let syscall_num = unsafe { *args };

    match syscall_num {
        SYS_HALT => halt(),

        SYS_EXIT => {
            check_args(args, 1);
            let status = unsafe { *args.add(1) } as i32;
            exit_process(status);
        }

        SYS_EXEC => {
            check_args(args, 1);
            let name_ptr = unsafe { *args.add(1) } as usize;
            if !is_valid_user_ptr(name_ptr, 1) {
                exit_process(-1);
            }
            let cmd = read_user_string(name_ptr);
            let tid = super::process::execute(&cmd);
            frame.eax = tid as u32;
        }

        SYS_WAIT => {
            check_args(args, 1);
            let tid = unsafe { *args.add(1) } as i32;
            frame.eax = super::process::wait(tid) as u32;
        }

        SYS_CREATE => {
            check_args(args, 2);
            let name_ptr = unsafe { *args.add(1) } as usize;
            let size = unsafe { *args.add(2) } as i32;
            if !is_valid_user_ptr(name_ptr, 1) {
                exit_process(-1);
            }
            let name = read_user_string(name_ptr);
            let ok = crate::filesys::filesys::create(&name, size);
            frame.eax = if ok { 1 } else { 0 };
        }

        SYS_REMOVE => {
            check_args(args, 1);
            let name_ptr = unsafe { *args.add(1) } as usize;
            if !is_valid_user_ptr(name_ptr, 1) {
                exit_process(-1);
            }
            let name = read_user_string(name_ptr);
            frame.eax = if crate::filesys::filesys::remove(&name) { 1 } else { 0 };
        }

        SYS_OPEN => {
            check_args(args, 1);
            let name_ptr = unsafe { *args.add(1) } as usize;
            if !is_valid_user_ptr(name_ptr, 1) {
                exit_process(-1);
            }
            let name = read_user_string(name_ptr);
            match crate::filesys::filesys::open(&name) {
                Some(file) => {
                    let fd_table = super::process::get_fd_table();
                    let fd = fd_table.open(file);
                    frame.eax = fd as u32;
                }
                None => {
                    frame.eax = (-1i32) as u32;
                }
            }
        }

        SYS_FILESIZE => {
            check_args(args, 1);
            let fd = unsafe { *args.add(1) } as i32;
            let fd_table = super::process::get_fd_table();
            match fd_table.get(fd) {
                Some(file) => {
                    frame.eax = file.length() as u32;
                }
                None => {
                    frame.eax = (-1i32) as u32;
                }
            }
        }

        SYS_READ => {
            check_args(args, 3);
            let fd = unsafe { *args.add(1) } as i32;
            let buf = unsafe { *args.add(2) } as usize;
            let size = unsafe { *args.add(3) } as usize;
            // Guard against absurd sizes that would wrap the address space.
            if size > PHYS_BASE {
                exit_process(-1);
            }
            frame.eax = sys_read(fd, buf, size) as u32;
        }

        SYS_WRITE => {
            check_args(args, 3);
            let fd = unsafe { *args.add(1) } as i32;
            let buf = unsafe { *args.add(2) } as usize;
            let size = unsafe { *args.add(3) } as usize;
            // Guard against absurd sizes that would wrap the address space.
            if size > PHYS_BASE {
                exit_process(-1);
            }
            frame.eax = sys_write(fd, buf, size) as u32;
        }

        SYS_SEEK => {
            check_args(args, 2);
            let fd = unsafe { *args.add(1) } as i32;
            let pos = unsafe { *args.add(2) } as i32;
            let fd_table = super::process::get_fd_table();
            if let Some(file) = fd_table.get(fd) {
                file.seek(pos);
            }
        }

        SYS_TELL => {
            check_args(args, 1);
            let fd = unsafe { *args.add(1) } as i32;
            let fd_table = super::process::get_fd_table();
            match fd_table.get(fd) {
                Some(file) => {
                    frame.eax = file.tell() as u32;
                }
                None => {
                    frame.eax = 0;
                }
            }
        }

        SYS_CLOSE => {
            check_args(args, 1);
            let fd = unsafe { *args.add(1) } as i32;
            let fd_table = super::process::get_fd_table();
            fd_table.close(fd);
        }

        SYS_MMAP => {
            check_args(args, 2);
            let fd = unsafe { *args.add(1) } as i32;
            let addr = unsafe { *args.add(2) } as usize;
            frame.eax = sys_mmap(fd, addr) as u32;
        }

        SYS_MUNMAP => {
            check_args(args, 1);
            let mapid = unsafe { *args.add(1) } as i32;
            sys_munmap(mapid);
        }

        SYS_MKDIR => {
            check_args(args, 1);
            let path_ptr = unsafe { *args.add(1) } as usize;
            if !is_valid_user_ptr(path_ptr, 1) {
                exit_process(-1);
            }
            let path = read_user_string(path_ptr);
            frame.eax = if crate::filesys::filesys::mkdir(&path) { 1 } else { 0 };
        }

        SYS_READDIR => {
            check_args(args, 2);
            let fd = unsafe { *args.add(1) } as i32;
            let name_ptr = unsafe { *args.add(2) } as usize;
            // Validate buffer can hold NAME_MAX + 1 bytes (max dir entry name + null).
            validate_user_buffer(name_ptr, crate::filesys::directory::NAME_MAX + 1);
            // Get the inode sector from the fd table, open as directory, and read next entry
            let fd_table = super::process::get_fd_table();
            match fd_table.get(fd) {
                Some(file) => {
                    let sector = file.inode_sector;
                    if !crate::filesys::inode::is_dir(sector) {
                        frame.eax = 0;
                    } else {
                        // Use the file's position as directory position
                        let pos = file.tell();
                        let mut dir = crate::filesys::directory::Dir { inode_sector: sector, pos };
                        // Don't open/close inode again - just borrow the sector
                        match dir.readdir() {
                            Some(name) => {
                                // Update the file position
                                file.seek(dir.pos);
                                // Write name to user buffer (already validated above)
                                let name_bytes = name.as_bytes();
                                let buf = unsafe {
                                    core::slice::from_raw_parts_mut(
                                        name_ptr as *mut u8,
                                        name_bytes.len() + 1,
                                    )
                                };
                                buf[..name_bytes.len()].copy_from_slice(name_bytes);
                                buf[name_bytes.len()] = 0;
                                frame.eax = 1;
                            }
                            None => {
                                frame.eax = 0;
                            }
                        }
                    }
                }
                None => {
                    frame.eax = 0;
                }
            }
        }

        SYS_ISDIR => {
            check_args(args, 1);
            let fd = unsafe { *args.add(1) } as i32;
            let fd_table = super::process::get_fd_table();
            match fd_table.get(fd) {
                Some(file) => {
                    frame.eax = if crate::filesys::inode::is_dir(file.inode_sector) { 1 } else { 0 };
                }
                None => {
                    frame.eax = 0;
                }
            }
        }

        SYS_INUMBER => {
            check_args(args, 1);
            let fd = unsafe { *args.add(1) } as i32;
            let fd_table = super::process::get_fd_table();
            match fd_table.get(fd) {
                Some(file) => {
                    frame.eax = file.inode_sector;
                }
                None => {
                    frame.eax = (-1i32) as u32;
                }
            }
        }

        _ => {
            crate::kprintln!("Unknown syscall: {}", syscall_num);
            exit_process(-1);
        }
    }
}
