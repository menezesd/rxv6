//! cal - print a calendar.
//!
//! Usage: cal           (current month - uses uptime as seed)
//!        cal 3 2024   (March 2024)
//!        cal 2024     (full year)

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let arg = |i: usize| -> &str {
        unsafe { rxv6_user::cstr_to_str(*argv.add(i)) }
    };

    if argc == 3 {
        // cal month year
        let month = atoi(arg(1)) as u32;
        let year = atoi(arg(2)) as u32;
        if month < 1 || month > 12 || year < 1 {
            println!("usage: cal [month year]");
            return 1;
        }
        print_month(year, month);
    } else if argc == 2 {
        // cal year - print full year
        let year = atoi(arg(1)) as u32;
        if year < 1 { println!("usage: cal [month year]"); return 1; }
        println!("                           {}", year);
        println!();
        for m in 1..=12 {
            print_month(year, m);
            println!();
        }
    } else {
        // Default: show a sample month (January 2024 as default since no real clock)
        // Use uptime to pick something semi-random, or just show Jan 2024
        let ticks = syscall::uptime();
        let month = ((ticks as u32 / 100) % 12) + 1;
        let year = 2024;
        print_month(year, month);
    }
    0
}

fn print_month(year: u32, month: u32) {
    let names = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December"
    ];
    let name = names[(month - 1) as usize];

    // Center the header
    let header_len = name.len() + 5; // "Month YYYY"
    let pad = if header_len < 20 { (20 - header_len) / 2 } else { 0 };
    for _ in 0..pad { print!(" "); }
    println!("{} {}", name, year);
    println!("Su Mo Tu We Th Fr Sa");

    let days = days_in_month(year, month);
    let start_dow = day_of_week(year, month, 1); // 0=Sunday

    // Gregorian reform: October 1582, days 5-14 don't exist
    let is_reform_month = year == 1582 && month == 10;

    // Leading spaces
    for _ in 0..start_dow {
        print!("   ");
    }

    let mut col = start_dow;
    for day in 1..=days {
        // Skip the removed days in October 1582
        if is_reform_month && day >= 5 && day <= 14 {
            continue;
        }
        if day < 10 { print!(" "); }
        print!("{}", day);
        if col == 6 { println!(); } else { print!(" "); }
        col = (col + 1) % 7;
    }
    if col != 0 { println!(); }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(year) { 29 } else { 28 },
        _ => 30,
    }
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Day of week for any date (0=Sunday).
/// Uses Gregorian formula for dates >= Oct 15, 1582; Julian formula before.
fn day_of_week(year: u32, month: u32, day: u32) -> u32 {
    let gregorian = year > 1582 || (year == 1582 && (month > 10 || (month == 10 && day >= 15)));

    // Adjust for Zeller: Jan/Feb are months 13/14 of previous year
    let (y, m) = if month <= 2 {
        (year as i32 - 1, month as i32 + 12)
    } else {
        (year as i32, month as i32)
    };
    let d = day as i32;
    let k = y % 100;
    let j = y / 100;

    let h = if gregorian {
        // Gregorian Zeller's congruence
        (d + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7
    } else {
        // Julian Zeller's congruence (no j/4 - 2j correction)
        (d + (13 * (m + 1)) / 5 + k + k / 4 + 5 - j) % 7
    };
    // h: 0=Sat, 1=Sun, 2=Mon, ... -> convert to 0=Sun
    ((h + 6) % 7) as u32
}

fn atoi(s: &str) -> i32 {
    let mut n: i32 = 0;
    for &b in s.as_bytes() {
        if b < b'0' || b > b'9' { break; }
        n = n * 10 + (b - b'0') as i32;
    }
    n
}
