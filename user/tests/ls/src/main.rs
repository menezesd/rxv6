//! ls - list directory contents.

#![no_std]
#![no_main]

use rxv6_user::syscall::{self, Stat, FileType};
use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc <= 1 {
        ls(b".\0".as_ptr());
    } else {
        for i in 1..argc {
            let path = unsafe { *argv.add(i as usize) };
            ls(path);
        }
    }
    0
}

fn ls(path: *const u8) {
    let fd = syscall::open(path, syscall::O_RDONLY);
    if fd < 0 {
        let name = unsafe { rxv6_user::cstr_to_str(path) };
        println!("ls: cannot open {}", name);
        return;
    }

    let mut st = Stat { file_type: FileType::None, dev: 0, ino: 0, nlink: 0, size: 0 };
    if syscall::fstat(fd, &mut st) < 0 {
        println!("ls: cannot stat");
        syscall::close(fd);
        return;
    }

    match st.file_type {
        FileType::File => {
            let name = unsafe { rxv6_user::cstr_to_str(path) };
            println!("{} {} {} {}", name, type_str(st.file_type), st.ino, st.size);
        }
        FileType::Dir => {
            // Read directory entries
            // For now, just print the directory's info
            let name = unsafe { rxv6_user::cstr_to_str(path) };
            println!("{} dir {} {}", name, st.ino, st.size);
            // TODO: read directory entries when readdir syscall is available
        }
        _ => {}
    }

    syscall::close(fd);
}

fn type_str(t: FileType) -> &'static str {
    match t {
        FileType::Dir => "dir",
        FileType::File => "file",
        FileType::Dev => "dev",
        FileType::None => "?",
    }
}
