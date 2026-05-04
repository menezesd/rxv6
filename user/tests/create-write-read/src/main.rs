#![no_std]
#![no_main]

use rxv6_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(create-write-read) begin");

    // Create a file
    if !syscall::create(b"test.txt\0", 0) {
        println!("(create-write-read) create failed");
        return 1;
    }

    // Open it
    let fd = syscall::open(b"test.txt\0");
    if fd < 0 {
        println!("(create-write-read) open failed");
        return 1;
    }

    // Write to it
    let msg = b"Hello filesystem!";
    let written = syscall::write(fd, msg);
    println!("(create-write-read) wrote {} bytes", written);

    // Seek back to beginning
    syscall::seek(fd, 0);

    // Read it back
    let mut buf = [0u8; 32];
    let nread = syscall::read(fd, &mut buf);
    if nread > 0 {
        let s = core::str::from_utf8(&buf[..nread as usize]).unwrap_or("???");
        println!("(create-write-read) read {} bytes: '{}'", nread, s);
    } else {
        println!("(create-write-read) read failed: {}", nread);
    }

    syscall::close(fd);
    println!("(create-write-read) end");
    0
}
