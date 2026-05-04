//! Kernel pipe implementation (xv6-style).
//!
//! A pipe is a bounded ring buffer with separate read and write file
//! descriptors. Readers block when empty; writers block when full.
//! EOF is signaled when all write ends are closed.

use alloc::boxed::Box;
use crate::sync::{Lock, Condvar};

const PIPE_SIZE: usize = 512;

/// Kernel pipe structure.
pub struct Pipe {
    lock: Lock,
    not_empty: Condvar,
    not_full: Condvar,
    buf: [u8; PIPE_SIZE],
    nread: usize,   // total bytes read
    nwrite: usize,  // total bytes written
    read_open: bool,
    write_open: bool,
}

impl Pipe {
    /// Allocate a new pipe on the heap.
    pub fn new() -> Box<Self> {
        let layout = core::alloc::Layout::new::<Self>();
        unsafe {
            let ptr = alloc::alloc::alloc_zeroed(layout) as *mut Self;
            assert!(!ptr.is_null(), "pipe: allocation failed");
            (*ptr).lock = Lock::new();
            (*ptr).not_empty = Condvar::new();
            (*ptr).not_full = Condvar::new();
            (*ptr).read_open = true;
            (*ptr).write_open = true;
            Box::from_raw(ptr)
        }
    }

    /// Write up to `n` bytes from `data` into the pipe.
    /// Blocks if pipe is full. Returns bytes written, or -1 if read end closed.
    pub fn write(&mut self, data: &[u8]) -> i32 {
        self.lock.acquire();
        let mut written = 0usize;
        for &byte in data {
            // Wait while pipe is full
            while self.nwrite == self.nread + PIPE_SIZE {
                if !self.read_open {
                    self.lock.release();
                    return -1;
                }
                self.not_full.wait(&mut self.lock);
            }
            self.buf[self.nwrite % PIPE_SIZE] = byte;
            self.nwrite += 1;
            written += 1;
            // Wake readers after each byte group
        }
        self.not_empty.signal(&self.lock);
        self.lock.release();
        written as i32
    }

    /// Read up to `n` bytes from the pipe into `buf`.
    /// Blocks if pipe is empty and write end is open.
    /// Returns 0 on EOF (write end closed, pipe empty).
    pub fn read(&mut self, buf: &mut [u8]) -> i32 {
        self.lock.acquire();
        // Wait while pipe is empty
        while self.nread == self.nwrite {
            if !self.write_open {
                self.lock.release();
                return 0; // EOF
            }
            self.not_empty.wait(&mut self.lock);
        }
        let mut read_count = 0usize;
        while read_count < buf.len() && self.nread < self.nwrite {
            buf[read_count] = self.buf[self.nread % PIPE_SIZE];
            self.nread += 1;
            read_count += 1;
        }
        self.not_full.signal(&self.lock);
        self.lock.release();
        read_count as i32
    }

    /// Close the read end of the pipe.
    pub fn close_read(&mut self) {
        self.lock.acquire();
        self.read_open = false;
        self.not_full.signal(&self.lock); // wake blocked writers
        self.lock.release();
    }

    /// Close the write end of the pipe.
    pub fn close_write(&mut self) {
        self.lock.acquire();
        self.write_open = false;
        self.not_empty.signal(&self.lock); // wake blocked readers
        self.lock.release();
    }
}

/// Global pipe registry. Pipes are identified by a unique ID.
/// Both read and write FDs reference the same pipe by ID.
static mut PIPES: Option<alloc::collections::BTreeMap<u32, Box<Pipe>>> = None;
static mut NEXT_PIPE_ID: u32 = 1;

pub fn init() {
    unsafe {
        PIPES = Some(alloc::collections::BTreeMap::new());
    }
}

fn pipes() -> &'static mut alloc::collections::BTreeMap<u32, Box<Pipe>> {
    static_mut!(PIPES)
}

/// Create a new pipe. Returns the pipe ID.
pub fn create_pipe() -> u32 {
    let pipe = Pipe::new();
    let id = unsafe { let id = NEXT_PIPE_ID; NEXT_PIPE_ID += 1; id };
    pipes().insert(id, pipe);
    id
}

/// Get a mutable reference to a pipe by ID.
pub fn get_pipe(id: u32) -> Option<&'static mut Pipe> {
    pipes().get_mut(&id).map(|p| &mut **p)
}

/// Remove a pipe from the registry (called when both ends are closed).
pub fn destroy_pipe(id: u32) {
    pipes().remove(&id);
}
