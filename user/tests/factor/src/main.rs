//! factor - print prime factors of a number.
//!
//! Usage: factor 12      -> 12: 2 2 3
//!        factor 97      -> 97: 97
//!        factor          (interactive: reads numbers from stdin)

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    if argc > 1 {
        for i in 1..argc as usize {
            let s = unsafe { rxv6_user::cstr_to_str(*argv.add(i)) };
            let n = atoi(s);
            if n < 1 {
                println!("{}: not a positive integer", s);
            } else {
                factorize(n as u64);
            }
        }
    } else {
        // Interactive mode
        let mut buf = [0u8; 64];
        loop {
            let n = getline(&mut buf);
            if n <= 0 { break; }
            let len = buf.iter().position(|&b| b == 0 || b == b'\n').unwrap_or(n as usize);
            if len == 0 { continue; }
            let s = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
            let num = atoi(s);
            if num < 1 { println!("{}: not valid", s); continue; }
            factorize(num as u64);
        }
    }
    0
}

fn factorize(n: u64) {
    print!("{}:", n);
    if n <= 1 {
        println!(" {}", n);
        return;
    }

    // Extract all prime factors using trial division + Pollard's rho
    let mut factors = [0u64; 64];
    let count = extract_factors_into(n, &mut factors);
    for i in 0..count {
        print!(" {}", factors[i]);
    }
    println!();
}

/// Pollard's rho with Brent's improvement.
/// Finds a non-trivial factor of n (assumes n is composite).
fn pollard_rho(n: u64) -> u64 {
    if n % 2 == 0 { return 2; }

    // Try multiple starting values
    let mut c: u64 = 1;
    loop {
        let mut x: u64 = 2;
        let mut y: u64 = 2;
        let mut d: u64 = 1;

        while d == 1 {
            x = mul_mod(x, x, n).wrapping_add(c) % n;
            y = mul_mod(y, y, n).wrapping_add(c) % n;
            y = mul_mod(y, y, n).wrapping_add(c) % n;
            d = gcd(abs_diff(x, y), n);
        }

        if d != n {
            return d;
        }
        c += 1;
        if c > 20 {
            // Fallback to trial division for stubborn cases
            let mut d = 3u64;
            while d * d <= n {
                if n % d == 0 { return d; }
                d += 2;
            }
            return n;
        }
    }
}

/// Extract all prime factors of n into buf, sorted. Returns count.
fn extract_factors_into(mut n: u64, buf: &mut [u64; 64]) -> usize {
    let mut count = 0;
    let mut d: u64 = 2;
    while d * d <= n && d < 1000 {
        while n % d == 0 {
            if count < 64 { buf[count] = d; count += 1; }
            n /= d;
        }
        d += if d == 2 { 1 } else { 2 };
    }
    while n > 1 {
        if is_prime(n) {
            if count < 64 { buf[count] = n; count += 1; }
            break;
        }
        let f = pollard_rho(n);
        if is_prime(f) {
            while n % f == 0 {
                if count < 64 { buf[count] = f; count += 1; }
                n /= f;
            }
        } else {
            // Recurse on the factor
            let mut sub = [0u64; 64];
            let sc = extract_factors_into(f, &mut sub);
            for i in 0..sc {
                while n % sub[i] == 0 {
                    if count < 64 { buf[count] = sub[i]; count += 1; }
                    n /= sub[i];
                }
            }
        }
    }
    // Sort
    for i in 1..count {
        let mut j = i;
        while j > 0 && buf[j] < buf[j-1] { buf.swap(j, j-1); j -= 1; }
    }
    count
}

/// Miller-Rabin primality test (deterministic for n < 2^64).
fn is_prime(n: u64) -> bool {
    if n < 2 { return false; }
    if n < 4 { return true; }
    if n % 2 == 0 || n % 3 == 0 { return false; }

    // Write n-1 as 2^r * d
    let mut d = n - 1;
    let mut r = 0u32;
    while d % 2 == 0 { d /= 2; r += 1; }

    // Witnesses sufficient for all n < 2^64
    let witnesses: [u64; 7] = [2, 3, 5, 7, 11, 13, 17];
    for &a in &witnesses {
        if a >= n { continue; }
        if !miller_test(a, d, n, r) { return false; }
    }
    true
}

fn miller_test(a: u64, d: u64, n: u64, r: u32) -> bool {
    let mut x = pow_mod(a, d, n);
    if x == 1 || x == n - 1 { return true; }
    for _ in 0..r - 1 {
        x = mul_mod(x, x, n);
        if x == n - 1 { return true; }
    }
    false
}

/// Modular exponentiation: base^exp mod m
fn pow_mod(mut base: u64, mut exp: u64, m: u64) -> u64 {
    let mut result: u64 = 1;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base, m);
        }
        exp >>= 1;
        base = mul_mod(base, base, m);
    }
    result
}

/// Modular multiplication avoiding overflow: (a * b) % m
/// Uses 128-bit intermediate (Rust supports this even on 32-bit targets).
fn mul_mod(a: u64, b: u64, m: u64) -> u64 {
    ((a as u128 * b as u128) % m as u128) as u64
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 { let t = b; b = a % b; a = t; }
    a
}

fn abs_diff(a: u64, b: u64) -> u64 {
    if a > b { a - b } else { b - a }
}

fn atoi(s: &str) -> i64 {
    let mut n: i64 = 0;
    let mut neg = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' { i += 1; }
    if i < bytes.len() && bytes[i] == b'-' { neg = true; i += 1; }
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        n = n * 10 + (bytes[i] - b'0') as i64;
        i += 1;
    }
    if neg { -n } else { n }
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
