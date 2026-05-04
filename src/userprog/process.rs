//! Process management: ELF loading, user stack setup, and process lifecycle.
//!
//! This module handles loading ELF binaries into user address spaces and
//! managing the transition between kernel and user mode.

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::filesys::file::File;
use crate::mem::vaddr::*;
use crate::sync::Semaphore;
use crate::thread;
use super::{pagedir, tss};

// ---- Process state (shared between parent and child) ----------------------

/// Shared state for parent-child wait synchronization.
#[allow(dead_code)]
pub struct ProcessState {
    pub tid: thread::Tid,
    pub exit_status: i32,
    pub exited: bool,
    pub waited: bool,
    pub wait_sema: Semaphore,   // parent blocks on this
    pub load_sema: Semaphore,   // parent waits for child load
    pub load_success: bool,
    pub executable_sector: Option<u32>,  // inode sector of running executable
}

/// Global registry of process states, keyed by child TID.
static mut PROCESS_STATES: Option<BTreeMap<thread::Tid, Box<ProcessState>>> = None;

// ---- Mmap state (global, keyed by TID) ------------------------------------

/// An entry in the per-process mmap table.
#[allow(dead_code)]
pub struct MmapEntry {
    pub mapid: i32,
    pub fd: i32,
    pub inode_sector: u32,
    pub uaddr: usize,
    pub length: usize,
}

/// Global mmap table: TID -> list of mmap entries.
static mut MMAP_TABLE: Option<BTreeMap<i32, Vec<MmapEntry>>> = None;

/// Next unique mmap id to hand out.
static mut NEXT_MAPID: i32 = 1;

/// Global SPT registry: TID -> SPT pointer. Avoids putting the pointer
/// in Thread struct (which would shrink the 4KB kernel stack).
static mut SPT_TABLE: Option<BTreeMap<i32, *mut crate::vm::page::SupplementaryPageTable>> = None;

fn spt_table() -> &'static mut BTreeMap<i32, *mut crate::vm::page::SupplementaryPageTable> {
    static_mut!(SPT_TABLE)
}

pub fn set_spt(tid: i32, spt: *mut crate::vm::page::SupplementaryPageTable) {
    spt_table().insert(tid, spt);
}

pub fn get_spt(tid: i32) -> *mut crate::vm::page::SupplementaryPageTable {
    spt_table().get(&tid).copied().unwrap_or_default()
}

pub fn remove_spt(tid: i32) -> *mut crate::vm::page::SupplementaryPageTable {
    spt_table().remove(&tid).unwrap_or_default()
}

/// Allocate a unique mmap id.
pub fn allocate_mapid() -> i32 {
    unsafe {
        let id = NEXT_MAPID;
        NEXT_MAPID += 1;
        id
    }
}

/// Record an mmap entry for the given thread.
pub fn add_mmap(tid: i32, entry: MmapEntry) {
    unsafe {
        if let Some(ref mut table) = MMAP_TABLE {
            table.entry(tid).or_insert_with(Vec::new).push(entry);
        }
    }
}

/// Remove and return the mmap entry with the given mapid for the given thread.
pub fn remove_mmap(tid: i32, mapid: i32) -> Option<MmapEntry> {
    unsafe {
        if let Some(ref mut table) = MMAP_TABLE {
            if let Some(entries) = table.get_mut(&tid) {
                if let Some(pos) = entries.iter().position(|e| e.mapid == mapid) {
                    return Some(entries.remove(pos));
                }
            }
        }
    }
    None
}

/// Check whether any existing mmap for this thread overlaps [addr, addr+len).
pub fn mmap_overlaps(tid: i32, addr: usize, len: usize) -> bool {
    unsafe {
        if let Some(ref table) = MMAP_TABLE {
            if let Some(entries) = table.get(&tid) {
                for e in entries {
                    let e_end = e.uaddr + e.length;
                    let new_end = addr + len;
                    // Overlaps if ranges intersect
                    if addr < e_end && new_end > e.uaddr {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Unmap a single mmap entry: write back dirty pages, unmap, free frames.
pub fn munmap_entry(entry: &MmapEntry) {
    let t = crate::thread::running_thread();
    let pd = unsafe { (*t).pagedir };
    if pd.is_null() {
        return;
    }

    let mut offset = 0usize;
    let mut cur_addr = entry.uaddr;

    while offset < entry.length {
        let kpage = pagedir::get_page(pd, cur_addr);
        if !kpage.is_null() {
            // Write back dirty pages
            if pagedir::is_dirty(pd, cur_addr) {
                let write_bytes = (entry.length - offset).min(PGSIZE);
                let buf = unsafe { core::slice::from_raw_parts(kpage, write_bytes) };
                crate::filesys::inode::write_at(
                    entry.inode_sector,
                    buf,
                    write_bytes as i32,
                    offset as i32,
                );
            }
            pagedir::clear_page(pd, cur_addr);
            let frame_addr = (kpage as usize & !(PGSIZE - 1)) as *mut u8;
            crate::vm::frame::free_frame(frame_addr);
        }
        offset += PGSIZE;
        cur_addr += PGSIZE;
    }
}

/// Unmap all mmaps for the given thread (called on process exit).
pub fn munmap_all(tid: i32) {
    unsafe {
        if let Some(ref mut table) = MMAP_TABLE {
            if let Some(entries) = table.remove(&tid) {
                for entry in &entries {
                    munmap_entry(entry);
                    // Close the inode we reopened during mmap
                    crate::filesys::inode::close(entry.inode_sector);
                }
            }
        }
    }
}

/// Initialize the process state registry. Call once at boot.
pub fn init() {
    unsafe {
        PROCESS_STATES = Some(BTreeMap::new());
        SPT_TABLE = Some(BTreeMap::new());
        MMAP_TABLE = Some(BTreeMap::new());
    }
}

fn process_states() -> &'static mut BTreeMap<thread::Tid, Box<ProcessState>> {
    static_mut!(PROCESS_STATES)
}

// ---- ELF structures -------------------------------------------------------

/// ELF32 file header.
#[repr(C)]
#[allow(dead_code)]
struct Elf32Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u32,
    e_phoff: u32,
    e_shoff: u32,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

/// ELF32 program header.
#[repr(C)]
#[allow(dead_code)]
struct Elf32Phdr {
    p_type: u32,
    p_offset: u32,
    p_vaddr: u32,
    p_paddr: u32,
    p_filesz: u32,
    p_memsz: u32,
    p_flags: u32,
    p_align: u32,
}

const PT_LOAD: u32 = 1;
const PF_W: u32 = 2;

/// ELF magic bytes.
const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

// ---- FD table -------------------------------------------------------------

/// File descriptor table for a user process.
pub struct FdTable {
    files: Vec<Option<Box<File>>>,
}

impl FdTable {
    /// Create a new FD table with stdin (0) and stdout (1) reserved.
    pub fn new() -> Box<Self> {
        let mut ft = Box::new(FdTable { files: Vec::new() });
        ft.files.push(None); // fd 0 = stdin (handled specially)
        ft.files.push(None); // fd 1 = stdout (handled specially)
        ft
    }

    /// Insert a file and return the assigned fd (>= 2).
    pub fn open(&mut self, file: Box<File>) -> i32 {
        // Find first empty slot starting from fd 2
        for i in 2..self.files.len() {
            if self.files[i].is_none() {
                self.files[i] = Some(file);
                return i as i32;
            }
        }
        let fd = self.files.len() as i32;
        self.files.push(Some(file));
        fd
    }

    /// Get a mutable reference to the file at `fd`.
    pub fn get(&mut self, fd: i32) -> Option<&mut File> {
        if fd < 2 { return None; }
        self.files.get_mut(fd as usize)?.as_mut().map(|f| &mut **f)
    }

    /// Close the file at `fd`. Returns true if it was open.
    pub fn close(&mut self, fd: i32) -> bool {
        if fd < 2 { return false; }
        if let Some(slot) = self.files.get_mut(fd as usize) {
            if let Some(file) = slot.take() {
                file.close_file();
                return true;
            }
        }
        false
    }

    /// Close all open files (used on process exit).
    pub fn close_all(&mut self) {
        for slot in self.files.iter_mut().skip(2) {
            if let Some(file) = slot.take() {
                file.close_file();
            }
        }
    }

    /// Duplicate a file descriptor (UNIX dup). Returns new fd, or -1.
    pub fn dup(&mut self, fd: i32) -> i32 {
        if fd < 0 { return -1; }
        if (fd as usize) >= self.files.len() { return -1; }

        // For stdin/stdout/stderr (0, 1, 2), just allocate a new slot
        // that refers to the same special fd (handled in syscall dispatch).
        if fd < 2 {
            // Dup stdin/stdout: find empty slot and mark it as a dup
            // For now, just return a new fd number (the syscall handler
            // will route reads/writes for fds 0/1 specially).
            return -1; // TODO: proper stdin/stdout dup
        }

        match &self.files[fd as usize] {
            Some(file) => {
                // Re-open the same inode to get a new File handle
                let sector = file.inode_sector;
                match File::open(sector) {
                    Some(new_file) => self.open(new_file),
                    None => -1,
                }
            }
            None => -1,
        }
    }
}

// ---- Process activation ----------------------------------------------------

/// Activate the current thread's page directory and update the TSS.
///
/// Called on every context switch so the CPU uses the correct address
/// space and kernel stack for the new thread.
pub fn activate() {
    let pd = current_pagedir();
    pagedir::activate(pd);
    tss::update();
}

/// Return the current thread's page directory, or null if it's a kernel thread.
fn current_pagedir() -> *mut u32 {
    let t = thread::running_thread();
    unsafe { (*t).pagedir }
}

// ---- Embedded ELF lookup ---------------------------------------------------

// Look up an embedded test ELF by program name.
// Returns Some(elf_data) if found, None otherwise.
// Embedded ELF lookup removed -- all programs are loaded from the filesystem.

// ---- Process execution -----------------------------------------------------

/// Execute a user program. Returns the new thread's TID, or TID_ERROR.
/// Loads the program from the filesystem.
pub fn execute(cmd_line: &str) -> thread::Tid {
    // Copy command line to a heap-allocated String
    let cmd_copy = String::from(cmd_line);

    // Extract program name (first word)
    let prog_name = cmd_line.split_whitespace().next().unwrap_or("");

    // Leak the String into a raw pointer so start_process can reclaim it
    let cmd_ptr = Box::into_raw(Box::new(cmd_copy)) as *mut u8;
    let tid = thread::create(prog_name, thread::PRI_DEFAULT, start_process, cmd_ptr);

    if tid == thread::TID_ERROR {
        // Reclaim on failure
        unsafe { drop(Box::from_raw(cmd_ptr as *mut String)); }
        return thread::TID_ERROR;
    }

    // Create process state for synchronization
    let ps = Box::new(ProcessState {
        tid,
        exit_status: -1,
        exited: false,
        waited: false,
        wait_sema: Semaphore::new(0),
        load_sema: Semaphore::new(0),
        load_success: false,
        executable_sector: None,
    });
    process_states().insert(tid, ps);

    // Wait for child to finish loading.
    // Safety: we just inserted this tid above, so get_mut must succeed.
    let ps = process_states().get_mut(&tid)
        .expect("execute: process state missing after insert");
    ps.load_sema.down();

    let load_ok = process_states().get(&tid).map(|p| p.load_success).unwrap_or(false);
    if !load_ok {
        // Child failed to load; clean up
        process_states().remove(&tid);
        return thread::TID_ERROR;
    }

    tid
}

/// Entry point for a new user process thread.
fn start_process(aux: *mut u8) {
    let cmd = unsafe { *Box::from_raw(aux as *mut String) };
    let tid = unsafe { (*thread::running_thread()).tid };

    // Load the ELF binary
    match load(&cmd) {
        Some((entry, esp)) => {
            // Signal load success
            if let Some(ps) = process_states().get_mut(&tid) {
                ps.load_success = true;
                ps.load_sema.up();
            }

            // Set up the FD table for this process
            let t = thread::running_thread();
            let fd_table = FdTable::new();
            unsafe { (*t).fd_table = Box::into_raw(fd_table); }

            // Activate the user page directory before jumping
            let pd = unsafe { (*t).pagedir };
            pagedir::activate(pd);

            // Jump to user mode
            jump_to_user(entry, esp);
        }
        None => {
            crate::kprintln!("process: failed to load '{}'", cmd);
            // Signal load failure
            if let Some(ps) = process_states().get_mut(&tid) {
                ps.load_success = false;
                ps.load_sema.up();
            }
            thread::exit();
        }
    }
}

/// Wait for a child process to exit and return its exit status.
pub fn wait(tid: thread::Tid) -> i32 {
    // Check if this TID exists in our registry
    if !process_states().contains_key(&tid) {
        return -1;
    }

    // Check if already waited on.
    // Safety: contains_key check above guarantees these lookups succeed.
    {
        let ps = process_states().get(&tid)
            .expect("wait: process state vanished");
        if ps.waited {
            return -1;
        }
    }

    // Mark as waited
    process_states().get_mut(&tid)
        .expect("wait: process state vanished").waited = true;

    // Block until child exits
    process_states().get_mut(&tid)
        .expect("wait: process state vanished").wait_sema.down();

    // Retrieve exit status and clean up
    let ps = process_states().remove(&tid)
        .expect("wait: process state vanished during wait");
    ps.exit_status
}

/// Wait for any child of `parent_tid` to exit (UNIX wait semantics).
/// Returns (child_pid, exit_status). Returns (-1, -1) if no children.
pub fn wait_any_child(_parent_tid: thread::Tid) -> (i32, i32) {
    // For now, find any process state that has exited or wait for one.
    // This is a simplified version -- a full implementation needs
    // parent-child tracking.
    let states = process_states();
    // Find first child that has exited
    let mut exited_tid = None;
    for (&tid, ps) in states.iter() {
        if ps.exited && !ps.waited {
            exited_tid = Some(tid);
            break;
        }
    }

    if let Some(tid) = exited_tid {
        let status = wait(tid);
        return (tid, status);
    }

    // No exited children yet -- find first unwaited child and block on it
    let first_child = {
        let mut found = None;
        for (&tid, ps) in states.iter() {
            if !ps.waited {
                found = Some(tid);
                break;
            }
        }
        found
    };

    match first_child {
        Some(tid) => {
            let status = wait(tid);
            (tid, status)
        }
        None => (-1, -1),
    }
}

/// Called when the current process exits.
#[allow(dead_code)]
pub fn exit() {
    exit_with_status(-1);
}

/// Called when the current process exits with a specific status.
pub fn exit_with_status(status: i32) {
    let t = thread::running_thread();
    let tid = unsafe { (*t).tid };

    // Allow writes to executable and close the inode
    if let Some(ps) = process_states().get_mut(&tid) {
        if let Some(sector) = ps.executable_sector.take() {
            crate::filesys::inode::allow_write(sector);
            crate::filesys::inode::close(sector);
        }
    }

    // Unmap all mmap'd regions (write back dirty pages) before destroying page dir.
    munmap_all(tid);

    // Record exit status and signal parent
    if let Some(ps) = process_states().get_mut(&tid) {
        ps.exit_status = status;
        ps.exited = true;
        ps.wait_sema.up();
    }

    // Note: child ProcessState cleanup happens in wait() after
    // the parent retrieves the exit status. States for children
    // that were never waited on are leaked (acceptable for an
    // educational OS - a production OS would track parent-child
    // relationships and clean up on parent exit).

    unsafe {
        // Clean up FD table
        if !(*t).fd_table.is_null() {
            let mut fd_table = Box::from_raw((*t).fd_table);
            fd_table.close_all();
            (*t).fd_table = core::ptr::null_mut();
        }

        // Destroy supplementary page table (frees swap slots for evicted pages).
        let spt_ptr = remove_spt((*t).tid);
        if !spt_ptr.is_null() {
            let mut spt = Box::from_raw(spt_ptr);
            spt.destroy();
        }

        // Destroy page directory
        let pd = (*t).pagedir;
        if !pd.is_null() {
            (*t).pagedir = core::ptr::null_mut();
            pagedir::activate(core::ptr::null_mut());
            pagedir::destroy(pd);
        }
    }
}

// ---- ELF loading -----------------------------------------------------------

/// Load an ELF binary from the filesystem into a new address space.
///
/// Returns `Some((entry_point, stack_pointer))` on success, `None` on failure.
pub fn load(cmdline: &str) -> Option<(u32, u32)> {
    // Extract filename (first token)
    let filename = cmdline.split_whitespace().next()?;

    // Open the executable file and read entirely into memory.
    // This simplifies ELF loading (avoids complex file-seek-based segment loading).
    let mut file = crate::filesys::filesys::open(filename)?;
    let file_len = file.length() as usize;
    let mut elf_data = vec![0u8; file_len];
    file.seek(0);
    let n = file.read(&mut elf_data, file_len as i32);
    if (n as usize) < file_len {
        file.close_file();
        return None;
    }

    // Delegate to the demand-paged loader
    let exec_sector = file.inode_sector;
    let result = load_demand_paged(&elf_data, cmdline, exec_sector);

    if result.is_some() {
        // Deny writes to the executable file while it's running.
        // Keep the inode open (don't close the file) so deny_write_cnt stays active.
        let exec_sector = file.inode_sector;
        crate::filesys::inode::deny_write(exec_sector);
        let tid = unsafe { (*thread::running_thread()).tid };
        if let Some(ps) = process_states().get_mut(&tid) {
            ps.executable_sector = Some(exec_sector);
        }
        // Forget the file object to keep the inode open_cnt elevated.
        // On exit, we will allow_write and close the inode directly.
        core::mem::forget(file);
    } else {
        file.close_file();
    }

    result
}

/// Load an ELF binary using demand paging: parse headers and create SPT entries,
/// but do NOT load segment data into frames. Pages are faulted in on first access.
fn load_demand_paged(elf_data: &[u8], cmdline: &str, inode_sector: u32) -> Option<(u32, u32)> {
    let ehdr_size = core::mem::size_of::<Elf32Ehdr>();
    if elf_data.len() < ehdr_size {
        return None;
    }

    // Copy header into properly aligned memory
    let mut ehdr_buf = Elf32Ehdr {
        e_ident: [0; 16], e_type: 0, e_machine: 0, e_version: 0,
        e_entry: 0, e_phoff: 0, e_shoff: 0, e_flags: 0,
        e_ehsize: 0, e_phentsize: 0, e_phnum: 0, e_shentsize: 0,
        e_shnum: 0, e_shstrndx: 0,
    };
    unsafe {
        core::ptr::copy_nonoverlapping(
            elf_data.as_ptr(),
            &mut ehdr_buf as *mut Elf32Ehdr as *mut u8,
            ehdr_size,
        );
    }
    let ehdr = &ehdr_buf;

    // Validate ELF
    if ehdr.e_ident[0..4] != ELF_MAGIC { return None; }
    if ehdr.e_type != 2 || ehdr.e_machine != 3 { return None; }

    let pd = pagedir::create();
    if pd.is_null() { return None; }

    // Create the supplementary page table for this process.
    let spt = Box::into_raw(Box::new(crate::vm::page::SupplementaryPageTable::new()));
    let t = thread::running_thread();
    let tid = unsafe { (*t).tid };
    set_spt(tid, spt);

    let old_pd = current_pagedir();
    pagedir::activate(pd);

    let entry = ehdr.e_entry;
    let mut success = true;

    // Parse PT_LOAD segments, collecting their ranges for SPT creation.
    // We need per-page writable flags (not a single global flag) to correctly
    // protect read-only .text/.rodata pages.
    struct SegInfo {
        vaddr: usize,
        filesz: usize,
        memsz: usize,
        file_off: usize,
        writable: bool,
    }
    let phdr_size = core::mem::size_of::<Elf32Phdr>();
    let mut segments: Vec<SegInfo> = Vec::new();

    for i in 0..ehdr.e_phnum as u32 {
        let off = (ehdr.e_phoff + i * ehdr.e_phentsize as u32) as usize;
        if off + phdr_size > elf_data.len() { success = false; break; }

        let mut phdr = Elf32Phdr {
            p_type: 0, p_offset: 0, p_vaddr: 0, p_paddr: 0,
            p_filesz: 0, p_memsz: 0, p_flags: 0, p_align: 0,
        };
        unsafe {
            core::ptr::copy_nonoverlapping(
                elf_data.as_ptr().add(off),
                &mut phdr as *mut Elf32Phdr as *mut u8,
                phdr_size,
            );
        }
        if phdr.p_type != PT_LOAD { continue; }
        if phdr.p_vaddr as usize >= PHYS_BASE { success = false; break; }
        if phdr.p_memsz < phdr.p_filesz { success = false; break; }

        segments.push(SegInfo {
            vaddr: phdr.p_vaddr as usize,
            filesz: phdr.p_filesz as usize,
            memsz: phdr.p_memsz as usize,
            file_off: phdr.p_offset as usize,
            writable: (phdr.p_flags & PF_W) != 0,
        });
    }

    if success && !segments.is_empty() {
        // Compute overall virtual range and file mapping base.
        let load_base_vaddr = segments.iter().map(|s| s.vaddr).min().unwrap();
        let load_base_foff = segments.iter().find(|s| s.vaddr == load_base_vaddr).unwrap().file_off;
        let load_end_vaddr = segments.iter().map(|s| s.vaddr + s.memsz).max().unwrap();
        let load_end_file = segments.iter().map(|s| s.vaddr + s.filesz).max().unwrap();

        // Create SPT entries page by page for the entire mapped region.
        let mut page = load_base_vaddr & !PGMASK;
        while page < load_end_vaddr {
            let spt_ref = unsafe { &mut *spt };
            if spt_ref.find(page).is_none() {
                let page_end = page + PGSIZE;

                // File data covers vaddr range [load_base_vaddr .. load_end_file)
                let file_start_in_page = page.max(load_base_vaddr);
                let file_end_in_page = page_end.min(load_end_file);

                let (read_bytes, page_ofs, file_offset) = if file_end_in_page > file_start_in_page {
                    let rb = file_end_in_page - file_start_in_page;
                    let po = file_start_in_page - page;
                    let fo = load_base_foff + (file_start_in_page - load_base_vaddr);
                    (rb, po, fo)
                } else {
                    (0, 0, 0)
                };

                // A page is writable if ANY overlapping segment has the write flag.
                // This is per-page, not global — read-only .text pages stay read-only.
                let writable = segments.iter().any(|seg| {
                    let seg_end = seg.vaddr + seg.memsz;
                    seg.vaddr < page_end && seg_end > page && seg.writable
                });

                let location = if read_bytes > 0 {
                    crate::vm::page::PageLocation::OnDisk
                } else {
                    crate::vm::page::PageLocation::Zero
                };

                spt_ref.insert(crate::vm::page::SptEntry {
                    vaddr: page,
                    location,
                    page_type: crate::vm::page::PageType::Binary,
                    writable,
                    inode_sector,
                    file_offset,
                    read_bytes,
                    page_ofs,
                });
            }
            page += PGSIZE;
        }
    }

    // Set up the initial stack page (eagerly — we need to push args onto it).
    let esp = if success { setup_stack(pd, cmdline) } else { None };

    // Add SPT entry for the stack page.
    if esp.is_some() {
        let stack_page = PHYS_BASE - PGSIZE;
        let spt_ref = unsafe { &mut *spt };
        spt_ref.insert(crate::vm::page::SptEntry {
            vaddr: stack_page,
            location: crate::vm::page::PageLocation::InMemory,
            page_type: crate::vm::page::PageType::Anonymous,
            writable: true,
            inode_sector: 0,
            file_offset: 0,
            read_bytes: 0,
            page_ofs: 0,
        });
    }

    pagedir::activate(if old_pd.is_null() { core::ptr::null_mut() } else { old_pd });

    let esp = match esp {
        Some(v) if success => v,
        _ => {
            // Clean up SPT on failure.
            remove_spt(tid);
            unsafe {
                let mut spt_box = Box::from_raw(spt);
                spt_box.destroy();
            }
            pagedir::destroy(pd);
            return None;
        }
    };

    unsafe { (*t).pagedir = pd; }

    Some((entry, esp))
}

// ---- User stack setup -------------------------------------------------------

/// Set up the initial user stack at the top of the user address space.
///
/// Allocates one page just below PHYS_BASE and pushes the argument
/// strings, argv array, argc, and a fake return address.
/// Returns the initial stack pointer on success.
fn setup_stack(pd: *mut u32, cmdline: &str) -> Option<u32> {
    let upage = PHYS_BASE - PGSIZE;
    let tid = unsafe { (*crate::thread::running_thread()).tid };
    let kpage = crate::vm::frame::alloc_frame(upage, tid);
    if kpage.is_null() {
        return None;
    }
    if !install_page(pd, upage, kpage as usize, true) {
        crate::vm::frame::free_frame(kpage);
        return None;
    }

    // Parse command line into args
    let args: Vec<&str> = cmdline.split_whitespace().collect();
    let argc = args.len();

    // Place strings at top of page, working downward
    let mut str_ptr = PHYS_BASE; // user VA, grows down
    let mut kstr_ptr = (kpage as usize + PGSIZE) as *mut u8; // kernel VA mirror
    let mut arg_addrs: Vec<u32> = Vec::with_capacity(argc);

    // Push arg strings in reverse order so arg_addrs ends up in forward order after reverse
    for arg in args.iter().rev() {
        let len = arg.len() + 1; // include null terminator
        str_ptr -= len;
        kstr_ptr = unsafe { kstr_ptr.sub(len) };
        unsafe {
            core::ptr::copy_nonoverlapping(arg.as_bytes().as_ptr(), kstr_ptr, arg.len());
            *kstr_ptr.add(arg.len()) = 0; // null terminator
        }
        arg_addrs.push(str_ptr as u32);
    }
    arg_addrs.reverse(); // back to forward order: arg_addrs[0] -> first arg

    // Word-align
    str_ptr &= !3;

    // Build stack frame (push downward)
    let mut sp = str_ptr;

    // argv[argc] = NULL sentinel
    sp -= 4;
    write_stack_word(kpage, sp - upage, 0);

    // argv[0..argc] pointers (push in reverse order)
    for i in (0..argc).rev() {
        sp -= 4;
        write_stack_word(kpage, sp - upage, arg_addrs[i]);
    }

    let argv_addr = sp as u32;

    // argv pointer (points to argv[0])
    sp -= 4;
    write_stack_word(kpage, sp - upage, argv_addr);

    // argc
    sp -= 4;
    write_stack_word(kpage, sp - upage, argc as u32);

    // Fake return address
    sp -= 4;
    write_stack_word(kpage, sp - upage, 0);

    Some(sp as u32)
}

/// Write a 32-bit value at an offset within a kernel page.
fn write_stack_word(kpage: *mut u8, offset: usize, value: u32) {
    unsafe {
        let ptr = kpage.add(offset) as *mut u32;
        *ptr = value;
    }
}

/// Map user virtual page `upage` to kernel page `kpage` in `pd`.
fn install_page(pd: *mut u32, upage: usize, kpage: usize, writable: bool) -> bool {
    // Verify upage is not already mapped.
    if !pagedir::get_page(pd, upage).is_null() {
        return false;
    }
    pagedir::set_page(pd, upage, kpage, writable)
}

// ---- Jump to user mode ------------------------------------------------------

/// Build an interrupt frame on the kernel stack and use `intr_exit` to
/// switch to ring 3 user mode.
///
/// The stack layout must match what `intr_exit` in intr_stubs.S expects:
///   low addr -> [popal regs] [gs fs es ds] [vec_no err_code frame_ptr] [eip cs eflags esp ss]
fn jump_to_user(eip: u32, esp: u32) -> ! {
    unsafe {
        core::arch::asm!(
            // Build the frame that intr_exit will pop (push in reverse order).
            // --- iret frame (popped by iret) ---
            "pushl $0x23",       // ss = SEL_UDSEG
            "pushl {esp_val}",   // user ESP
            "pushl $0x202",      // EFLAGS = IF | reserved bit 1
            "pushl $0x1b",       // cs = SEL_UCSEG
            "pushl {eip_val}",   // user EIP
            // --- stub fields (skipped by addl $12, %esp) ---
            "pushl $0",          // frame_pointer
            "pushl $0",          // error_code
            "pushl $0",          // vec_no
            // --- segment registers (popl ds, es, fs, gs) ---
            "pushl $0x23",       // ds = SEL_UDSEG
            "pushl $0x23",       // es = SEL_UDSEG
            "pushl $0x23",       // fs = SEL_UDSEG
            "pushl $0x23",       // gs = SEL_UDSEG
            // --- general registers (popal) ---
            "pushl $0",          // eax
            "pushl $0",          // ecx
            "pushl $0",          // edx
            "pushl $0",          // ebx
            "pushl $0",          // esp_dummy (ignored by popal)
            "pushl $0",          // ebp
            "pushl $0",          // esi
            "pushl $0",          // edi
            "jmp intr_exit",
            eip_val = in(reg) eip,
            esp_val = in(reg) esp,
            options(att_syntax, noreturn)
        );
    }
}

// ---- Helper for getting FD table from current thread -----------------------

/// Get a mutable reference to the current thread's FD table.
/// Panics if the current thread has no FD table (kernel thread).
pub fn get_fd_table() -> &'static mut FdTable {
    let t = thread::running_thread();
    unsafe {
        let ptr = (*t).fd_table;
        assert!(!ptr.is_null(), "get_fd_table: no FD table (kernel thread?)");
        &mut *ptr
    }
}
