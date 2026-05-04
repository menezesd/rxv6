#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::{println, syscall};

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(page-merge-par) begin");
    let pid = syscall::exec(b"child-simple\0" as *const u8);
    // Do merge while child runs
    let mut data = [0u32; 128];
    for i in 0..128 {
        data[i] = (128 - i) as u32;
    }
    // Sort (bubble)
    for i in 0..128 {
        for j in 0..127 - i {
            if data[j] > data[j + 1] {
                data.swap(j, j + 1);
            }
        }
    }
    let s = syscall::wait(pid);
    let mut ok = true;
    for i in 0..128 {
        if data[i] != (i + 1) as u32 {
            ok = false;
            break;
        }
    }
    if ok && s == 81 {
        println!("(page-merge-par) PASSED");
    }
    0
}
