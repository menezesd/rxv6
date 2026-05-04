#![no_std]
#![no_main]

extern crate rxv6_user;
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(page-shuffle) begin");
    let mut data = [0u8; 1024];
    // Fill with pattern
    for i in 0..1024 {
        data[i] = (i & 0xFF) as u8;
    }
    // Shuffle access
    for i in (0..1024).step_by(7) {
        let j = (i * 13 + 5) % 1024;
        data.swap(i, j);
    }
    // Verify no corruption
    let mut sum: u32 = 0;
    for &b in &data {
        sum = sum.wrapping_add(b as u32);
    }
    println!("(page-shuffle) PASSED (sum={})", sum);
    0
}
