//! A fixed-size bitmap that lives in pre-allocated (non-heap) memory.
//!
//! Used by the page allocator to track free/used pages.

pub struct Bitmap {
    bits: *mut u8,
    bit_cnt: usize,
}

// Safety: Bitmap is only accessed with interrupts disabled (via InterruptGuard).
unsafe impl Send for Bitmap {}
unsafe impl Sync for Bitmap {}

impl Bitmap {
    /// Create an empty (invalid) bitmap. Must call `create_in_buf` before use.
    pub const fn empty() -> Self {
        Bitmap {
            bits: core::ptr::null_mut(),
            bit_cnt: 0,
        }
    }

    /// Initialise a bitmap of `bit_cnt` bits inside the buffer at `buf`.
    ///
    /// All bits are initially `false` (0).
    ///
    /// # Safety
    /// `buf` must point to at least `buf_size(bit_cnt)` writable bytes that
    /// remain valid for the lifetime of the bitmap.
    pub unsafe fn create_in_buf(bit_cnt: usize, buf: *mut u8, buf_size: usize) -> Self {
        let needed = Self::byte_cnt(bit_cnt);
        assert!(buf_size >= needed, "bitmap buffer too small");
        // Zero the backing store.
        core::ptr::write_bytes(buf, 0, needed);
        Bitmap { bits: buf, bit_cnt }
    }

    /// Number of bytes required to store `bit_cnt` bits.
    pub fn byte_cnt(bit_cnt: usize) -> usize {
        bit_cnt.div_ceil(8)
    }

    /// Number of bits in this bitmap.
    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.bit_cnt
    }

    /// Set bit `idx` to `value`.
    pub fn set(&mut self, idx: usize, value: bool) {
        debug_assert!(idx < self.bit_cnt);
        unsafe {
            let byte = self.bits.add(idx / 8);
            let mask = 1u8 << (idx % 8);
            if value {
                *byte |= mask;
            } else {
                *byte &= !mask;
            }
        }
    }

    /// Return the value of bit `idx`.
    pub fn test(&self, idx: usize) -> bool {
        debug_assert!(idx < self.bit_cnt);
        unsafe {
            let byte = *self.bits.add(idx / 8);
            (byte >> (idx % 8)) & 1 != 0
        }
    }

    /// Set `cnt` bits starting at `start` to `value`.
    pub fn set_multiple(&mut self, start: usize, cnt: usize, value: bool) {
        for i in start..start + cnt {
            self.set(i, value);
        }
    }

    /// Check whether all `cnt` bits starting at `start` have the given `value`.
    #[allow(dead_code)]
    pub fn all(&self, start: usize, cnt: usize, value: bool) -> bool {
        for i in start..start + cnt {
            if self.test(i) != value {
                return false;
            }
        }
        true
    }

    /// Find the first run of `cnt` consecutive bits equal to `value`,
    /// starting the search at bit `start`. If found, flip them to `!value`
    /// and return the start index.
    pub fn scan_and_flip(&mut self, start: usize, cnt: usize, value: bool) -> Option<usize> {
        if cnt == 0 {
            return Some(start);
        }
        if start + cnt > self.bit_cnt {
            return None;
        }

        let mut i = start;
        while i + cnt <= self.bit_cnt {
            // Find first bit equal to `value`.
            if self.test(i) != value {
                i += 1;
                continue;
            }
            // Check if we have `cnt` consecutive bits.
            let mut run = 1;
            while run < cnt && self.test(i + run) == value {
                run += 1;
            }
            if run >= cnt {
                self.set_multiple(i, cnt, !value);
                return Some(i);
            }
            // Skip past the failed run.
            i += run + 1;
        }
        None
    }
}
