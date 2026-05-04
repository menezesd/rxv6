//! dc - desk calculator (RPN / reverse Polish notation).
//!
//! One of the original UNIX V6 programs.
//!
//! Usage: enter numbers and operators separated by spaces or newlines.
//!   Numbers are pushed onto the stack.
//!   Operators pop operands, push result.
//!
//! Operators:
//!   + - * / %    arithmetic (integer)
//!   p            print top of stack
//!   n            print top and pop (no newline)
//!   f            print entire stack
//!   c            clear stack
//!   d            duplicate top
//!   r            swap top two
//!   q            quit
//!   _N           negative number (e.g., _5 = -5)

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

const STACK_SIZE: usize = 256;

struct Dc {
    stack: [i64; STACK_SIZE],
    sp: usize, // stack pointer (next free slot)
}

impl Dc {
    fn new() -> Self { Dc { stack: [0; STACK_SIZE], sp: 0 } }

    fn push(&mut self, v: i64) {
        if self.sp >= STACK_SIZE {
            println!("dc: stack overflow");
            return;
        }
        self.stack[self.sp] = v;
        self.sp += 1;
    }

    fn pop(&mut self) -> Option<i64> {
        if self.sp == 0 {
            println!("dc: stack empty");
            return None;
        }
        self.sp -= 1;
        Some(self.stack[self.sp])
    }

    fn top(&self) -> Option<i64> {
        if self.sp == 0 { None } else { Some(self.stack[self.sp - 1]) }
    }
}

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut dc = Dc::new();
    let mut buf = [0u8; 512];

    loop {
        let n = getline(&mut buf);
        if n < 0 { break; }
        let len = n as usize;
        let mut i = 0;

        while i < len {
            // Skip whitespace
            while i < len && (buf[i] == b' ' || buf[i] == b'\t') { i += 1; }
            if i >= len { break; }

            let ch = buf[i];

            // Number (including _N for negative)
            if ch.is_ascii_digit() || (ch == b'_' && i + 1 < len && buf[i + 1].is_ascii_digit()) {
                let neg = ch == b'_';
                if neg { i += 1; }
                let mut num: i64 = 0;
                while i < len && buf[i].is_ascii_digit() {
                    num = num * 10 + (buf[i] - b'0') as i64;
                    i += 1;
                }
                if neg { num = -num; }
                dc.push(num);
                continue;
            }

            i += 1; // consume the operator character

            match ch {
                b'+' => {
                    if let (Some(b), Some(a)) = (dc.pop(), dc.pop()) {
                        dc.push(a + b);
                    }
                }
                b'-' => {
                    if let (Some(b), Some(a)) = (dc.pop(), dc.pop()) {
                        dc.push(a - b);
                    }
                }
                b'*' => {
                    if let (Some(b), Some(a)) = (dc.pop(), dc.pop()) {
                        dc.push(a * b);
                    }
                }
                b'/' => {
                    if let (Some(b), Some(a)) = (dc.pop(), dc.pop()) {
                        if b == 0 { println!("dc: divide by zero"); dc.push(a); }
                        else { dc.push(a / b); }
                    }
                }
                b'%' => {
                    if let (Some(b), Some(a)) = (dc.pop(), dc.pop()) {
                        if b == 0 { println!("dc: divide by zero"); dc.push(a); }
                        else { dc.push(a % b); }
                    }
                }
                b'p' => {
                    if let Some(v) = dc.top() { println!("{}", v); }
                    else { println!("dc: stack empty"); }
                }
                b'n' => {
                    if let Some(v) = dc.pop() { print!("{}", v); }
                }
                b'f' => {
                    for j in (0..dc.sp).rev() {
                        println!("{}", dc.stack[j]);
                    }
                }
                b'c' => { dc.sp = 0; }
                b'd' => {
                    if let Some(v) = dc.top() { dc.push(v); }
                }
                b'r' => {
                    if dc.sp >= 2 {
                        dc.stack.swap(dc.sp - 1, dc.sp - 2);
                    }
                }
                b'q' => { return 0; }
                _ => { println!("dc: unknown: {}", ch as char); }
            }
        }
    }
    0
}

fn getline(buf: &mut [u8]) -> i32 {
    let mut i = 0;
    while i < buf.len() - 1 {
        let mut c = [0u8; 1];
        let n = syscall::read(0, &mut c);
        if n <= 0 { return if i > 0 { i as i32 } else { -1 }; }
        if c[0] == b'\n' { break; }
        buf[i] = c[0];
        i += 1;
    }
    buf[i] = 0;
    i as i32
}
