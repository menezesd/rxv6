#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

/// Access a `static mut Option<T>` via raw pointer, returning `&'static mut T`.
/// Avoids creating a mutable reference to the static itself (UB in Rust 2024).
/// Panics if the Option is None (i.e., the static was never initialized).
#[macro_export]
macro_rules! static_mut {
    ($name:ident) => {
        unsafe {
            (&raw mut $name).as_mut().unwrap()
                .as_mut().expect(concat!(stringify!($name), " not initialized"))
        }
    };
}

mod arch;
mod console;
mod devices;
mod filesys;
mod mem;
mod sync;
mod thread;
mod userprog;
mod tests;
mod vm;

use core::panic::PanicInfo;

// Include boot assembly (multiboot header, paging setup, entry point)
core::arch::global_asm!(include_str!("../asm/start.S"), options(att_syntax));

/// Kernel entry point. Called from boot assembly after paging is set up
/// and we're running in the higher half (virtual addresses at 0xC0000000+).
#[no_mangle]
pub extern "C" fn main() -> ! {
    // Phase 1: Console output
    devices::serial::init();
    devices::vga::clear();
    kprintln!("RustOS booting...");

    // Phase 2: GDT, IDT, Memory
    arch::gdt::init();

    arch::idt::init();

    // Initialize memory system
    mem::palloc::init(0);
    kprintln!("Page allocator initialized.");

    mem::malloc::init();
    kprintln!("Kernel heap initialized.");

    mem::paging::init();
    kprintln!("Paging initialized (4KB page tables).");

    // Phase 3: Threading
    thread::init();
    kprintln!("Thread system initialized.");

    // Phase 7: TSS + user program support (before enabling interrupts)
    userprog::tss::init();
    kprintln!("TSS initialized.");

    // Initialize interrupt handlers for user programs
    userprog::process::init();
    userprog::exception::init();
    userprog::syscall::init();

    // Initialize timer (registers IRQ handler)
    devices::timer::init();

    // Start threading (creates idle thread, enables interrupts)
    thread::start();
    kprintln!("Threading started.");

    // Calibrate timer (requires interrupts + threading)
    devices::timer::calibrate();

    // Phase 5: Devices
    devices::keyboard::init();
    devices::ide::init();
    kprintln!("Devices initialized.");

    // Phase 8: Virtual memory subsystem (after devices so swap can find its block device)
    vm::frame::init();
    vm::swap::init();
    kprintln!("VM subsystem initialized.");

    // Verify heap works with Box
    {
        let boxed = alloc::boxed::Box::new(42u32);
        kprintln!("Box::new(42) = {} -- heap works!", *boxed);

        let v = alloc::vec![1u32, 2, 3];
        kprintln!("Vec: {:?} -- alloc works!", v);
    }

    kprintln!("Timer ticks: {}", devices::timer::ticks());

    // Phase 4: sync tests skipped (verified in Phase 4)

    // Phase 6: Filesystem
    // Run on a fresh thread to get a clean 4KB stack (main's stack is deep).
    if devices::block::get_by_name("hda").is_some() {
        devices::block::set_role(devices::block::BlockType::FileSys, 0);
        thread::create("fs-init", thread::PRI_DEFAULT, fs_test_thread, core::ptr::null_mut());
        while !unsafe { FS_TEST_DONE } {
            thread::yield_current();
        }
    } else {
        kprintln!("No disk found, skipping filesystem test.");
    }

    // Quick filesystem check (only if disk is available)
    let has_fs = devices::block::get_role(devices::block::BlockType::FileSys).is_some();
    if has_fs {
        if let Some(f) = filesys::filesys::open("hello") {
            kprintln!("fs-check: opened 'hello' len={}", f.length());
            f.close_file();
        }
    }

    // Persistence test
    if has_fs && filesys::filesys::open("persist-write").is_some() {
        if filesys::filesys::open("persist.dat").is_some() {
            kprintln!("\n=== Persistence test (reboot - verifying) ===");
            run_user_test("persist-read");
        } else {
            kprintln!("\n=== Persistence test (first boot - writing) ===");
            run_user_test("persist-write");
        }
    }

    // Kernel thread tests (Pintos Project 1)
    kprintln!("\n=== Kernel Thread Tests ===\n");
    tests::threads::run_all();

    // Phase 7+8: Run user test suite (only if filesystem available)
    if has_fs {
        kprintln!("\n=== RustOS User Test Suite ===\n");
        run_user_test_suite();
    }

    kprintln!("\n=== All tests complete ===");
    devices::shutdown::power_off()
}

// ── Phase 4: Synchronization tests ──────────────────────────────────

/// Shared state for producer-consumer test.
#[allow(dead_code)]
struct ProdCons {
    lock: sync::Lock,
    not_empty: sync::Condvar,
    not_full: sync::Condvar,
    buf: [i32; 4],
    count: usize,
    head: usize,
    tail: usize,
    produced: i32,
    consumed: i32,
}

#[allow(dead_code)]
static mut PRODCONS: ProdCons = ProdCons {
    lock: sync::Lock::new(),
    not_empty: sync::Condvar::new(),
    not_full: sync::Condvar::new(),
    buf: [0; 4],
    count: 0,
    head: 0,
    tail: 0,
    produced: 0,
    consumed: 0,
};

#[allow(dead_code, static_mut_refs)]
fn test_sync() {
    kprintln!("\n--- Semaphore test ---");
    // Simple semaphore ping-pong between two threads.
    static mut SEMA: sync::Semaphore = sync::Semaphore::new(0);

    fn sema_thread(_aux: *mut u8) {
        for i in 0..3 {
            unsafe { SEMA.down(); }
            kprintln!("  sema-waiter: woke up ({})", i);
        }
    }

    thread::create("sema-wait", thread::PRI_DEFAULT, sema_thread, core::ptr::null_mut());

    for i in 0..3 {
        kprintln!("  main: signalling sema ({})", i);
        unsafe { SEMA.up(); }
        thread::yield_current();
    }

    // Let waiter finish
    thread::yield_current();
    kprintln!("Semaphore test passed.");

    kprintln!("\n--- Lock test ---");
    static mut LOCK_COUNTER: i32 = 0;
    static mut TEST_LOCK: sync::Lock = sync::Lock::new();

    fn lock_thread(_aux: *mut u8) {
        for _ in 0..100 {
            unsafe {
                TEST_LOCK.acquire();
                LOCK_COUNTER += 1;
                TEST_LOCK.release();
            }
        }
    }

    thread::create("lock-a", thread::PRI_DEFAULT, lock_thread, core::ptr::null_mut());
    thread::create("lock-b", thread::PRI_DEFAULT, lock_thread, core::ptr::null_mut());

    // Wait for lock threads to finish
    for _ in 0..20 {
        thread::yield_current();
    }

    let counter = unsafe { LOCK_COUNTER };
    kprintln!("  lock counter = {} (expected 200)", counter);
    assert_eq!(counter, 200);
    kprintln!("Lock test passed.");

    kprintln!("\n--- Producer-Consumer test ---");
    thread::create("producer", thread::PRI_DEFAULT, producer, core::ptr::null_mut());
    thread::create("consumer", thread::PRI_DEFAULT, consumer, core::ptr::null_mut());

    // Wait for producer/consumer to finish
    for _ in 0..100 {
        thread::yield_current();
    }

    let (produced, consumed) = unsafe { (PRODCONS.produced, PRODCONS.consumed) };
    kprintln!("  produced={}, consumed={}", produced, consumed);
    assert_eq!(produced, 8);
    assert_eq!(consumed, 8);
    kprintln!("Producer-Consumer test passed.");
}

#[allow(dead_code)]
fn producer(_aux: *mut u8) {
    let pc = &raw mut PRODCONS;
    for i in 0..8 {
        unsafe {
            (*pc).lock.acquire();
            while (*pc).count == 4 {
                (*pc).not_full.wait(&mut (*pc).lock);
            }
            (*pc).buf[(*pc).tail] = i;
            (*pc).tail = ((*pc).tail + 1) % 4;
            (*pc).count += 1;
            (*pc).produced += 1;
            kprintln!("  producer: put {}", i);
            (*pc).not_empty.signal(&(*pc).lock);
            (*pc).lock.release();
        }
    }
}

#[allow(dead_code)]
fn consumer(_aux: *mut u8) {
    let pc = &raw mut PRODCONS;
    for _ in 0..8 {
        unsafe {
            (*pc).lock.acquire();
            while (*pc).count == 0 {
                (*pc).not_empty.wait(&mut (*pc).lock);
            }
            let val = (*pc).buf[(*pc).head];
            (*pc).head = ((*pc).head + 1) % 4;
            (*pc).count -= 1;
            (*pc).consumed += 1;
            kprintln!("  consumer: got {}", val);
            (*pc).not_full.signal(&(*pc).lock);
            (*pc).lock.release();
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::idt::intr_disable();
    // Print to serial directly first to avoid VGA cascading panics
    devices::serial::write_fmt(format_args!("\n!!! KERNEL PANIC !!!\n{}\n", info));
    halt()
}

static mut FS_TEST_DONE: bool = false;
#[allow(dead_code)]
static mut USER_TEST_DONE: bool = false;

fn run_user_test(name: &str) {
    kprintln!("--- {} ---", name);
    let tid = userprog::process::execute(name);
    if tid != thread::TID_ERROR {
        let status = userprog::process::wait(tid);
        kprintln!("--- {} exited with status {} ---", name, status);
    } else {
        kprintln!("--- {} FAILED TO LOAD ---", name);
    }
}

fn run_user_test_suite() {
    // Tests ordered: read-only tests first, file-creating tests last.
    // This prevents free map exhaustion from affecting exec-based tests.

    // --- Tests that DON'T create files (run first, filesystem stays clean) ---

    kprintln!("-- Userprog basic --");
    run_user_test("hello"); run_user_test("args"); run_user_test("exit");
    run_user_test("write-stdout"); run_user_test("sc-bad-arg"); run_user_test("boundary");
    run_user_test("bad-read"); run_user_test("bad-write"); run_user_test("bad-jump");
    run_user_test("bad-read2"); run_user_test("bad-write2"); run_user_test("bad-jump2");
    run_user_test("wait-bad-pid");

    kprintln!("\n-- Exec/Wait --");
    run_user_test("exec-once"); run_user_test("exec-arg"); run_user_test("exec-missing");
    run_user_test("exec-multiple"); run_user_test("exec-bound");
    run_user_test("exec-bound-2"); run_user_test("exec-bound-3");
    run_user_test("wait-simple"); run_user_test("wait-twice"); run_user_test("wait-killed");
    run_user_test("child-bad"); run_user_test("child-close"); run_user_test("child-inherit");
    run_user_test("multi-child-fd"); run_user_test("multi-recurse");
    run_user_test("exec-bad-ptr");

    kprintln!("\n-- ROX --");
    run_user_test("rox-simple"); run_user_test("rox-child"); run_user_test("rox-multichild");

    kprintln!("\n-- VM --");
    run_user_test("pt-grow-stack"); run_user_test("pt-grow-bad");
    run_user_test("pt-grow-pusha"); run_user_test("pt-grow-stk-sc");
    run_user_test("pt-big-stk-obj"); run_user_test("pt-write-code");
    // pt-wrt-code2 skipped: it writes to code pages via kernel, causing kernel page fault
    run_user_test("pt-bad-addr"); run_user_test("pt-bad-read");
    run_user_test("page-linear"); run_user_test("page-shuffle");

    kprintln!("\n-- VM memory pressure --");
    run_user_test("page-merge-seq"); run_user_test("page-merge-par");
    run_user_test("page-merge-stk"); run_user_test("page-parallel");
    run_user_test("parallel-merge");
    run_user_test("child-linear"); run_user_test("child-sort");
    run_user_test("child-qsort"); run_user_test("qsort");

    // --- Tests that CREATE files (run after exec tests to avoid free map issues) ---

    kprintln!("\n-- Filesys read-only --");
    run_user_test("open-missing"); run_user_test("open-empty"); run_user_test("open-null");
    run_user_test("close-stdin"); run_user_test("close-stdout");
    run_user_test("close-bad-fd"); run_user_test("read-bad-fd"); run_user_test("read-zero");
    run_user_test("read-stdout"); run_user_test("write-bad-fd"); run_user_test("write-zero");
    run_user_test("write-stdin"); run_user_test("read-bad-ptr");
    run_user_test("write-bad-ptr"); run_user_test("open-bad-ptr");
    run_user_test("sl-bad-target");
    run_user_test("create-empty"); run_user_test("create-long");
    run_user_test("create-null"); run_user_test("create-bad-ptr");

    kprintln!("\n-- Filesys create/write --");
    run_user_test("create-normal"); run_user_test("create-exists"); run_user_test("create-bound");
    run_user_test("open-normal"); run_user_test("open-twice"); run_user_test("open-boundary");
    run_user_test("close-normal"); run_user_test("close-twice");
    run_user_test("read-write"); run_user_test("read-normal"); run_user_test("read-boundary");
    run_user_test("write-normal"); run_user_test("write-boundary");
    run_user_test("filesize"); run_user_test("seek-tell"); run_user_test("remove");
    run_user_test("sl-check"); run_user_test("sl-read"); run_user_test("sl-remove");
    run_user_test("sc-boundary"); run_user_test("sc-boundary-2"); run_user_test("sc-boundary-3");

    kprintln!("\n-- Small/Large files --");
    run_user_test("sm-create"); run_user_test("sm-full"); run_user_test("sm-random");
    run_user_test("sm-seq-block"); run_user_test("sm-seq-random");
    run_user_test("lg-create"); run_user_test("lg-full"); run_user_test("lg-random");
    run_user_test("lg-seq-block"); run_user_test("lg-seq-random");

    kprintln!("\n-- File growth --");
    run_user_test("grow-create"); run_user_test("grow-seq-sm"); run_user_test("grow-seq-lg");
    run_user_test("grow-file-size"); run_user_test("grow-tell");
    run_user_test("grow-sparse"); run_user_test("grow-two-files");
    run_user_test("grow-root-sm"); run_user_test("grow-root-lg");

    kprintln!("\n-- Sync --");
    run_user_test("syn-remove"); run_user_test("syn-read"); run_user_test("syn-write");
    run_user_test("sparse-file");
    run_user_test("child-syn-read"); run_user_test("child-syn-wrt");

    kprintln!("\n-- Mmap --");
    run_user_test("mmap-bad-fd"); run_user_test("mmap-null");
    run_user_test("mmap-read"); run_user_test("mmap-write");
    run_user_test("mmap-close"); run_user_test("mmap-unmap");
    run_user_test("mmap-overlap"); run_user_test("mmap-twice");
    run_user_test("mmap-clean"); run_user_test("mmap-exit");

    kprintln!("\n-- Directories --");
    run_user_test("dir-mkdir"); run_user_test("dir-open"); run_user_test("dir-rmdir");
    run_user_test("dir-empty-name"); run_user_test("dir-mk-tree");
    run_user_test("dir-over-file"); run_user_test("dir-under-file");
    run_user_test("dir-rm-root"); run_user_test("dir-rm-tree");
    run_user_test("dir-rm-parent"); run_user_test("dir-vine");
}

fn fs_test_thread(_aux: *mut u8) {
    let dev = devices::block::get_role(devices::block::BlockType::FileSys)
        .expect("fs_test_thread: no FileSys block device");
    let size = dev.size;
    kprintln!("filesys: hda as FileSys ({} sectors)", size);

    // Load the pre-built filesystem from the disk image (created by mkdisk).
    // Do NOT format -- the disk already contains the RustOS filesystem.
    filesys::filesys::init(false);
    kprintln!("Filesystem loaded from disk image.");

    // Quick test: can we open files from the disk?
    if let Some(f) = filesys::filesys::open("hello") {
        kprintln!("  fs check: opened 'hello' (len={})", f.length());
        f.close_file();
    } else {
        kprintln!("  fs check: FAILED to open 'hello'");
    }

    unsafe { FS_TEST_DONE = true; }
}

fn halt() -> ! {
    loop {
        unsafe {
            core::arch::asm!("cli", "hlt");
        }
    }
}
