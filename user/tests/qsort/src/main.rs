#![no_std]
#![no_main]

extern crate rustos_user;
use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("(qsort) begin");
    let mut data: [u8; 100] = [0; 100];
    for i in 0..100 {
        data[i] = (100 - i) as u8;
    } // reverse order
    // Simple bubble sort (no qsort in no_std)
    for i in 0..100 {
        for j in 0..99 - i {
            if data[j] > data[j + 1] {
                data.swap(j, j + 1);
            }
        }
    }
    let mut ok = true;
    for i in 0..100 {
        if data[i] != (i + 1) as u8 {
            ok = false;
            break;
        }
    }
    if ok {
        println!("(qsort) PASSED");
    } else {
        println!("(qsort) FAILED");
    }
    0
}
