#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(page-merge-seq) begin");
    // Allocate two arrays, merge them
    let mut a = [0u32; 64];
    let mut b = [0u32; 64];
    for i in 0..64 {
        a[i] = (i * 2) as u32;
        b[i] = (i * 2 + 1) as u32;
    }
    let mut merged = [0u32; 128];
    let (mut ai, mut bi, mut mi) = (0, 0, 0);
    while ai < 64 && bi < 64 {
        if a[ai] <= b[bi] {
            merged[mi] = a[ai];
            ai += 1;
        } else {
            merged[mi] = b[bi];
            bi += 1;
        }
        mi += 1;
    }
    while ai < 64 {
        merged[mi] = a[ai];
        ai += 1;
        mi += 1;
    }
    while bi < 64 {
        merged[mi] = b[bi];
        bi += 1;
        mi += 1;
    }
    let mut ok = true;
    for i in 0..128 {
        if merged[i] != i as u32 {
            ok = false;
            break;
        }
    }
    if ok {
        println!("(page-merge-seq) PASSED");
    } else {
        println!("(page-merge-seq) FAILED");
    }
    0
}
