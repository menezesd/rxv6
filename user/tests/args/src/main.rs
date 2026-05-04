#![no_std]
#![no_main]

use rustos_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    println!("(args) begin");
    println!("(args) argc = {}", argc);
    for i in 0..=argc {
        let ptr = unsafe { *argv.add(i as usize) };
        if ptr.is_null() {
            println!("(args) argv[{}] = null", i);
        } else {
            // Read null-terminated C string
            let mut len = 0;
            unsafe {
                while *ptr.add(len) != 0 {
                    len += 1;
                }
            }
            let s = unsafe {
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
            };
            println!("(args) argv[{}] = '{}'", i, s);
        }
    }
    println!("(args) end");
    println!("(args) PASSED");
    0
}
