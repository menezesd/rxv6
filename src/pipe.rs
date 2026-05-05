//! Kernel pipe implementation (xv6-style).
//!
//! A pipe is a bounded ring buffer with separate read and write file
//! descriptors. Readers block when empty; writers block when full.
//! EOF is signaled when all write ends are closed.
//!
//! Each pipe end is reference-counted: dup() and fork() increment the
//! refcount, close() decrements it. The pipe end is only truly closed
//! (signaling EOF) when the last reference is dropped.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
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
    read_refcnt: u32,
    write_refcnt: u32,
}

impl Pipe {
    /// Allocate a new pipe.
    pub fn new() -> Box<Self> {
        Box::new(Pipe {
            lock: Lock::new(),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
            buf: [0; PIPE_SIZE],
            nread: 0,
            nwrite: 0,
            read_open: true,
            write_open: true,
            read_refcnt: 1,
            write_refcnt: 1,
        })
    }

    /// Write bytes into the pipe. Blocks if full.
    /// Copies in batches rather than byte-at-a-time.
    /// Returns bytes written, or -1 if read end closed (SIGPIPE).
    pub fn write(&mut self, data: &[u8]) -> i32 {
        self.lock.acquire();
        let mut written = 0usize;
        while written < data.len() {
            // Wait while pipe is full
            while self.nwrite == self.nread + PIPE_SIZE {
                if !self.read_open {
                    let already = written;
                    self.lock.release();
                    if already == 0 {
                        let t = crate::thread::running_thread();
                        let tid = unsafe { (*t).tid };
                        crate::thread::send_signal(tid, crate::thread::SIGPIPE);
                    }
                    return if already > 0 { already as i32 } else { -1 };
                }
                // Signal readers before blocking — they may need to drain
                // the pipe so we can continue writing.
                self.not_empty.signal(&self.lock);
                self.not_full.wait(&mut self.lock);
            }
            // Batch copy as many bytes as will fit
            let space = PIPE_SIZE - (self.nwrite - self.nread);
            let n = (data.len() - written).min(space);
            for i in 0..n {
                self.buf[(self.nwrite + i) % PIPE_SIZE] = data[written + i];
            }
            self.nwrite += n;
            written += n;
        }
        self.not_empty.signal(&self.lock);
        self.lock.release();
        written as i32
    }

    /// Read bytes from the pipe. Blocks if empty and write end is open.
    /// Copies in batches. Returns 0 on EOF.
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
        // Batch copy as many bytes as available
        let available = self.nwrite - self.nread;
        let n = buf.len().min(available);
        for i in 0..n {
            buf[i] = self.buf[(self.nread + i) % PIPE_SIZE];
        }
        self.nread += n;
        self.not_full.signal(&self.lock);
        self.lock.release();
        n as i32
    }

    /// Increment read-end reference count (for dup/fork).
    pub fn ref_read(&mut self) {
        self.lock.acquire();
        self.read_refcnt += 1;
        self.lock.release();
    }

    /// Increment write-end reference count (for dup/fork).
    pub fn ref_write(&mut self) {
        self.lock.acquire();
        self.write_refcnt += 1;
        self.lock.release();
    }

    /// Close a read-end reference. Returns true if both ends are fully closed.
    pub fn close_read(&mut self) -> bool {
        self.lock.acquire();
        self.read_refcnt -= 1;
        if self.read_refcnt == 0 {
            self.read_open = false;
            self.not_full.signal(&self.lock); // wake blocked writers
        }
        let both_closed = !self.read_open && !self.write_open;
        self.lock.release();
        both_closed
    }

    /// Close a write-end reference. Returns true if both ends are fully closed.
    pub fn close_write(&mut self) -> bool {
        self.lock.acquire();
        self.write_refcnt -= 1;
        if self.write_refcnt == 0 {
            self.write_open = false;
            self.not_empty.signal(&self.lock); // wake blocked readers
        }
        let both_closed = !self.read_open && !self.write_open;
        self.lock.release();
        both_closed
    }
}

/// Global pipe registry. Pipes are identified by a unique ID.
/// Both read and write FDs reference the same pipe by ID.
static mut PIPES: Option<BTreeMap<u32, Box<Pipe>>> = None;
static mut NEXT_PIPE_ID: u32 = 1;

pub fn init() {
    unsafe {
        PIPES = Some(BTreeMap::new());
    }
}

fn pipes() -> &'static mut BTreeMap<u32, Box<Pipe>> {
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
