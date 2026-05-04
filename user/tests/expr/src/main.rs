//! expr - evaluate arithmetic expressions.
//!
//! Usage: expr 3 + 5
//!        expr 10 \* 3    (shell requires escaping *)
//!        expr 7 / 2
//!        expr 10 % 3
//!        expr 5 = 5      (returns 1 if true, 0 if false)
//!        expr 3 \> 2
//!        expr 3 \< 2
//!
//! Each token is a separate argument. Prints the result.
//! Exit status: 0 if result is nonzero/non-null, 1 otherwise.

#![no_std]
#![no_main]

use rxv6_user::println;

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        println!("usage: expr operand [operator operand] ...");
        return 2;
    }

    // Simple: evaluate left to right (no precedence, like V7 expr with single op)
    let arg = |i: usize| -> &str {
        unsafe { rxv6_user::cstr_to_str(*argv.add(i)) }
    };

    // Single operand
    if argc == 2 {
        let val = atoi(arg(1));
        println!("{}", val);
        return if val != 0 { 0 } else { 1 };
    }

    // Evaluate: expr a op b [op c ...]
    let mut result = atoi(arg(1));
    let mut i = 2usize;
    while i + 1 < argc as usize {
        let op = arg(i);
        let right = atoi(arg(i + 1));
        result = match op {
            "+" => result + right,
            "-" => result - right,
            "*" => result * right,
            "/" => { if right == 0 { println!("expr: division by zero"); return 2; } result / right }
            "%" => { if right == 0 { println!("expr: division by zero"); return 2; } result % right }
            "=" => if result == right { 1 } else { 0 },
            "!=" => if result != right { 1 } else { 0 },
            "<" => if result < right { 1 } else { 0 },
            ">" => if result > right { 1 } else { 0 },
            "<=" => if result <= right { 1 } else { 0 },
            ">=" => if result >= right { 1 } else { 0 },
            _ => { println!("expr: unknown operator: {}", op); return 2; }
        };
        i += 2;
    }

    println!("{}", result);
    if result != 0 { 0 } else { 1 }
}

fn atoi(s: &str) -> i64 {
    let bytes = s.as_bytes();
    let mut n: i64 = 0;
    let mut neg = false;
    let mut i = 0;
    if i < bytes.len() && bytes[i] == b'-' { neg = true; i += 1; }
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        n = n * 10 + (bytes[i] - b'0') as i64;
        i += 1;
    }
    if neg { -n } else { n }
}
