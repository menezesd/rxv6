//! od - octal dump.
#![no_std]
#![no_main]
use rxv6_user::syscall;
use rxv6_user::print;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let fd = if argc > 1 {
        let path = unsafe { *argv.add(1) };
        let f = syscall::open(path, syscall::O_RDONLY);
        if f < 0 { rxv6_user::println!("od: cannot open"); return 1; }
        f
    } else { 0 };

    let mut buf = [0u8; 16];
    let mut offset = 0u32;
    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 { break; }
        print!("{:07o}", offset);
        for i in 0..n as usize {
            print!(" {:03o}", buf[i]);
        }
        print!("\n");
        offset += n as u32;
    }
    if fd > 0 { syscall::close(fd); }
    0
}
