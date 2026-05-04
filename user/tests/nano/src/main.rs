//! nano - a simple text editor for rxv6.
//!
//! Ctrl+O  Save       Ctrl+K  Cut line      Ctrl+W  Search
//! Ctrl+X  Exit       Ctrl+U  Paste line     Ctrl+G  Help
//! Ctrl+C  Cursor pos Ctrl+T  Go to line     Ctrl+\  Replace

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use alloc::vec;
use rxv6_user::syscall;

// ---- Constants ----

const TAB_WIDTH: usize = 4;

// ---- Buffer ----

struct Buffer {
    lines: Vec<Vec<u8>>,  // each line WITHOUT trailing \n
    filename: [u8; 128],
    filename_len: usize,
    dirty: bool,
    // Cursor
    cx: usize,  // column in current line
    cy: usize,  // row in buffer (0-indexed)
    // Scroll offset
    row_off: usize,
    col_off: usize,
    // Screen dimensions
    screen_rows: usize,
    screen_cols: usize,
    // Cut buffer (for Ctrl+K / Ctrl+U)
    cut_buf: Vec<Vec<u8>>,
    // Search state
    last_search: Vec<u8>,
    // Status message
    status_msg: [u8; 80],
    status_len: usize,
}

impl Buffer {
    fn new() -> Self {
        Buffer {
            lines: vec![Vec::new()], // start with one empty line
            filename: [0; 128],
            filename_len: 0,
            dirty: false,
            cx: 0,
            cy: 0,
            row_off: 0,
            col_off: 0,
            screen_rows: 22, // 25 - title - status - help
            screen_cols: 80,
            cut_buf: Vec::new(),
            last_search: Vec::new(),
            status_msg: [0; 80],
            status_len: 0,
        }
    }

    fn num_lines(&self) -> usize {
        self.lines.len()
    }

    fn current_line(&self) -> &Vec<u8> {
        &self.lines[self.cy]
    }

    fn current_line_mut(&mut self) -> &mut Vec<u8> {
        &mut self.lines[self.cy]
    }

    fn set_status(&mut self, msg: &[u8]) {
        let len = msg.len().min(self.status_msg.len());
        self.status_msg[..len].copy_from_slice(&msg[..len]);
        self.status_len = len;
    }

    fn set_filename(&mut self, name: &[u8]) {
        let len = name.len().min(127);
        self.filename[..len].copy_from_slice(&name[..len]);
        self.filename[len] = 0;
        self.filename_len = len;
    }
}

// ---- Terminal I/O ----

fn write_str(s: &[u8]) {
    syscall::write(1, s);
}

fn write_byte(b: u8) {
    syscall::write(1, &[b]);
}

fn read_key() -> Key {
    let mut c = [0u8; 1];
    let n = syscall::read(0, &mut c);
    if n <= 0 { return Key::Ctrl(b'x'); } // EOF = exit

    match c[0] {
        0x1B => {
            // Escape sequence
            let mut seq = [0u8; 1];
            if !syscall::input_ready() { return Key::Escape; }
            syscall::read(0, &mut seq);
            if seq[0] != b'[' { return Key::Escape; }
            if !syscall::input_ready() { return Key::Escape; }
            syscall::read(0, &mut seq);
            match seq[0] {
                b'A' => Key::Up,
                b'B' => Key::Down,
                b'C' => Key::Right,
                b'D' => Key::Left,
                b'H' => Key::Home,
                b'F' => Key::End,
                b'5' => { consume_tilde(); Key::PageUp }
                b'6' => { consume_tilde(); Key::PageDown }
                b'3' => { consume_tilde(); Key::Delete }
                b'2' => { consume_tilde(); Key::Insert }
                _ => Key::Escape,
            }
        }
        1..=26 => Key::Ctrl(c[0] + b'a' - 1), // Ctrl+A through Ctrl+Z
        0x7F => Key::Backspace,
        b'\r' | b'\n' => Key::Enter,
        b'\t' => Key::Tab,
        c => Key::Char(c),
    }
}

fn consume_tilde() {
    if syscall::input_ready() {
        let mut t = [0u8; 1];
        syscall::read(0, &mut t);
    }
}

#[derive(PartialEq)]
enum Key {
    Char(u8),
    Ctrl(u8),
    Enter,
    Backspace,
    Delete,
    Tab,
    Up, Down, Left, Right,
    Home, End,
    PageUp, PageDown,
    Insert,
    Escape,
}

// ---- Screen rendering ----

fn render(buf: &Buffer) {
    let mut out = Vec::with_capacity(2048);

    // Hide cursor, go home
    out.extend_from_slice(b"\x1b[?25l\x1b[H");

    // Title bar (inverted)
    out.extend_from_slice(b"\x1b[7m");
    let title = b"  rxv6 nano  ";
    out.extend_from_slice(title);
    // Filename
    if buf.filename_len > 0 {
        out.extend_from_slice(&buf.filename[..buf.filename_len]);
    } else {
        out.extend_from_slice(b"[new file]");
    }
    if buf.dirty {
        out.extend_from_slice(b" (modified)");
    }
    // Pad rest of title bar
    let title_used = title.len() + if buf.filename_len > 0 { buf.filename_len } else { 10 }
        + if buf.dirty { 11 } else { 0 };
    for _ in title_used..buf.screen_cols {
        out.push(b' ');
    }
    out.extend_from_slice(b"\x1b[0m\r\n");

    // Text area
    for screen_row in 0..buf.screen_rows {
        let file_row = screen_row + buf.row_off;
        out.extend_from_slice(b"\x1b[K"); // clear line

        if file_row < buf.num_lines() {
            let line = &buf.lines[file_row];
            // Render visible portion with tab expansion
            let mut col = 0;
            for &ch in line.iter() {
                if col >= buf.col_off + buf.screen_cols { break; }
                if ch == b'\t' {
                    let spaces = TAB_WIDTH - (col % TAB_WIDTH);
                    for _ in 0..spaces {
                        if col >= buf.col_off && col < buf.col_off + buf.screen_cols {
                            out.push(b' ');
                        }
                        col += 1;
                    }
                } else {
                    if col >= buf.col_off {
                        out.push(ch);
                    }
                    col += 1;
                }
            }
        } else {
            out.push(b'~');
        }
        out.extend_from_slice(b"\r\n");
    }

    // Status line (inverted)
    out.extend_from_slice(b"\x1b[7m");
    if buf.status_len > 0 {
        let len = buf.status_len.min(buf.screen_cols);
        out.extend_from_slice(&buf.status_msg[..len]);
        for _ in len..buf.screen_cols { out.push(b' '); }
    } else {
        // Default status: line/col info
        let info = format_status(buf);
        let len = info.len().min(buf.screen_cols);
        out.extend_from_slice(&info[..len]);
        for _ in len..buf.screen_cols { out.push(b' '); }
    }
    out.extend_from_slice(b"\x1b[0m\r\n");

    // Help bar
    out.extend_from_slice(b"\x1b[K\x1b[7m");
    let help = b"^O Save  ^X Exit  ^K Cut  ^U Paste  ^W Search  ^T GoTo  ^\\ Replace  ^G Help";
    let hlen = help.len().min(buf.screen_cols);
    out.extend_from_slice(&help[..hlen]);
    for _ in hlen..buf.screen_cols { out.push(b' '); }
    out.extend_from_slice(b"\x1b[0m");

    // Position cursor
    let screen_cy = buf.cy - buf.row_off + 1; // +1 for title bar
    let screen_cx = render_cx(buf) - buf.col_off;
    out.extend_from_slice(b"\x1b[");
    append_num(&mut out, screen_cy + 1); // 1-based
    out.push(b';');
    append_num(&mut out, screen_cx + 1);
    out.push(b'H');

    // Show cursor
    out.extend_from_slice(b"\x1b[?25h");

    write_str(&out);
}

/// Get rendered x position (accounting for tabs).
fn render_cx(buf: &Buffer) -> usize {
    let cy = buf.cy;
    let line = &buf.lines[cy];
    let mut rx = 0;
    for i in 0..buf.cx.min(line.len()) {
        if line[i] == b'\t' {
            rx += TAB_WIDTH - (rx % TAB_WIDTH);
        } else {
            rx += 1;
        }
    }
    rx
}

fn format_status(buf: &Buffer) -> [u8; 80] {
    let mut s = [b' '; 80];
    let mut i = 0;
    // "Line X/Y, Col Z"
    let msg = b"  Line ";
    s[..msg.len()].copy_from_slice(msg);
    i += msg.len();
    i += write_num(&mut s[i..], buf.cy + 1);
    s[i] = b'/';
    i += 1;
    i += write_num(&mut s[i..], buf.num_lines());
    let col_msg = b", Col ";
    s[i..i + col_msg.len()].copy_from_slice(col_msg);
    i += col_msg.len();
    i += write_num(&mut s[i..], buf.cx + 1);
    let _ = i;
    s
}

fn write_num(buf: &mut [u8], n: usize) -> usize {
    if n == 0 { buf[0] = b'0'; return 1; }
    let mut tmp = [0u8; 10];
    let mut len = 0;
    let mut val = n;
    while val > 0 { tmp[len] = b'0' + (val % 10) as u8; val /= 10; len += 1; }
    for j in 0..len { buf[j] = tmp[len - 1 - j]; }
    len
}

fn append_num(out: &mut Vec<u8>, n: usize) {
    let mut tmp = [0u8; 10];
    let len = write_num(&mut tmp, n);
    out.extend_from_slice(&tmp[..len]);
}

// ---- Scroll ----

fn scroll(buf: &mut Buffer) {
    // Vertical scrolling
    if buf.cy < buf.row_off {
        buf.row_off = buf.cy;
    }
    if buf.cy >= buf.row_off + buf.screen_rows {
        buf.row_off = buf.cy - buf.screen_rows + 1;
    }
    // Horizontal scrolling
    let rx = render_cx(buf);
    if rx < buf.col_off {
        buf.col_off = rx;
    }
    if rx >= buf.col_off + buf.screen_cols {
        buf.col_off = rx - buf.screen_cols + 1;
    }
}

// ---- Editing operations ----

fn insert_char(buf: &mut Buffer, c: u8) {
    if buf.cy >= buf.num_lines() {
        buf.lines.push(Vec::new());
    }
    let cy = buf.cy;
    if buf.cx > buf.lines[cy].len() { buf.cx = buf.lines[cy].len(); }
    let cx = buf.cx;
    buf.lines[cy].insert(cx, c);
    buf.cx += 1;
    buf.dirty = true;
}

fn insert_tab(buf: &mut Buffer) {
    insert_char(buf, b'\t');
}

fn insert_newline(buf: &mut Buffer) {
    let cy = buf.cy;
    let cx = buf.cx;
    let rest: Vec<u8> = if cx < buf.lines[cy].len() {
        buf.lines[cy][cx..].to_vec()
    } else {
        Vec::new()
    };
    buf.lines[cy].truncate(cx);
    buf.cy += 1;
    buf.lines.insert(buf.cy, rest);
    buf.cx = 0;
    buf.dirty = true;
}

fn delete_char(buf: &mut Buffer) {
    if buf.cy >= buf.num_lines() { return; }
    if buf.cx == 0 && buf.cy == 0 { return; }

    if buf.cx > 0 {
        let cy = buf.cy;
        let cx = buf.cx;
        if cx <= buf.lines[cy].len() {
            buf.lines[cy].remove(cx - 1);
            buf.cx -= 1;
            buf.dirty = true;
        }
    } else {
        // Join with previous line
        let current = buf.lines.remove(buf.cy);
        buf.cy -= 1;
        buf.cx = buf.lines[buf.cy].len();
        let cy = buf.cy;
        buf.lines[cy].extend_from_slice(&current);
        buf.dirty = true;
    }
}

fn delete_forward(buf: &mut Buffer) {
    if buf.cy >= buf.num_lines() { return; }
    let cy = buf.cy;
    let line_len = buf.lines[cy].len();

    if buf.cx < line_len {
        let cx = buf.cx;
        buf.lines[cy].remove(cx);
        buf.dirty = true;
    } else if buf.cy + 1 < buf.num_lines() {
        // Join with next line
        let next = buf.lines.remove(buf.cy + 1);
        let cy = buf.cy;
        buf.lines[cy].extend_from_slice(&next);
        buf.dirty = true;
    }
}

fn cut_line(buf: &mut Buffer) {
    if buf.cy >= buf.num_lines() { return; }
    let line = buf.lines.remove(buf.cy);
    buf.cut_buf.push(line);
    if buf.lines.is_empty() {
        buf.lines.push(Vec::new());
    }
    if buf.cy >= buf.num_lines() {
        buf.cy = buf.num_lines() - 1;
    }
    buf.cx = 0;
    buf.dirty = true;
}

fn paste_lines(buf: &mut Buffer) {
    if buf.cut_buf.is_empty() { return; }
    for (i, line) in buf.cut_buf.clone().iter().enumerate() {
        buf.lines.insert(buf.cy + 1 + i, line.clone());
    }
    buf.cy += buf.cut_buf.len();
    buf.cx = 0;
    buf.dirty = true;
}

// ---- File I/O ----

fn load_file(buf: &mut Buffer) {
    if buf.filename_len == 0 { return; }

    let fd = syscall::open(buf.filename.as_ptr(), syscall::O_RDONLY);
    if fd < 0 { return; }

    buf.lines.clear();
    let mut current_line = Vec::new();
    let mut read_buf = [0u8; 512];

    loop {
        let n = syscall::read(fd, &mut read_buf);
        if n <= 0 { break; }
        for &b in &read_buf[..n as usize] {
            if b == b'\n' {
                buf.lines.push(current_line);
                current_line = Vec::new();
            } else {
                current_line.push(b);
            }
        }
    }
    // Last line (may not end with \n)
    buf.lines.push(current_line);
    syscall::close(fd);

    buf.dirty = false;
    buf.cy = 0;
    buf.cx = 0;
    buf.row_off = 0;
    buf.col_off = 0;
}

fn save_file(buf: &mut Buffer) -> bool {
    if buf.filename_len == 0 {
        // Prompt for filename
        if let Some(name) = prompt(buf, b"File Name to Write: ") {
            if name.is_empty() { return false; }
            buf.set_filename(&name);
        } else {
            return false;
        }
    }

    let fd = syscall::open(buf.filename.as_ptr(),
        syscall::O_WRONLY | syscall::O_CREATE | syscall::O_TRUNC);
    if fd < 0 {
        buf.set_status(b"Error: cannot open file for writing");
        return false;
    }

    let mut total = 0usize;
    for (i, line) in buf.lines.iter().enumerate() {
        syscall::write(fd, line);
        total += line.len();
        if i + 1 < buf.lines.len() {
            syscall::write(fd, b"\n");
            total += 1;
        }
    }
    syscall::close(fd);
    buf.dirty = false;

    // Status message
    let mut msg = [0u8; 80];
    let prefix = b"Wrote ";
    msg[..prefix.len()].copy_from_slice(prefix);
    let mut i = prefix.len();
    i += write_num(&mut msg[i..], total);
    let suffix = b" bytes";
    msg[i..i + suffix.len()].copy_from_slice(suffix);
    i += suffix.len();
    buf.status_msg[..i].copy_from_slice(&msg[..i]);
    buf.status_len = i;
    true
}

// ---- Search ----

fn search(buf: &mut Buffer) {
    let default = if buf.last_search.is_empty() {
        Vec::new()
    } else {
        buf.last_search.clone()
    };

    let query = if let Some(q) = prompt(buf, b"Search: ") {
        if q.is_empty() {
            if default.is_empty() { return; }
            default
        } else {
            q
        }
    } else {
        return;
    };

    buf.last_search = query.clone();
    search_forward(buf, &query, buf.cy, buf.cx + 1);
}

fn search_forward(buf: &mut Buffer, query: &[u8], start_row: usize, start_col: usize) {
    // Search from current position forward, wrapping
    let n = buf.num_lines();
    for offset in 0..n {
        let row = (start_row + offset) % n;
        let line = &buf.lines[row];
        let from = if offset == 0 { start_col } else { 0 };
        if let Some(col) = find_in_line(line, query, from) {
            buf.cy = row;
            buf.cx = col;
            return;
        }
    }
    buf.set_status(b"Not found");
}

fn find_in_line(line: &[u8], query: &[u8], from: usize) -> Option<usize> {
    if query.is_empty() || query.len() > line.len() { return None; }
    let end = line.len() - query.len() + 1;
    for i in from..end {
        if &line[i..i + query.len()] == query {
            return Some(i);
        }
    }
    None
}

fn replace(buf: &mut Buffer) {
    let search_q = if let Some(q) = prompt(buf, b"Search (to replace): ") {
        if q.is_empty() { return; }
        q
    } else { return; };

    let replace_with = if let Some(r) = prompt(buf, b"Replace with: ") {
        r
    } else { return; };

    let mut count = 0u32;
    for row in 0..buf.num_lines() {
        let mut col = 0;
        loop {
            let line = &buf.lines[row];
            match find_in_line(line, &search_q, col) {
                Some(pos) => {
                    // Replace
                    let line = &mut buf.lines[row];
                    let end = pos + search_q.len();
                    let mut new_line = Vec::with_capacity(line.len());
                    new_line.extend_from_slice(&line[..pos]);
                    new_line.extend_from_slice(&replace_with);
                    new_line.extend_from_slice(&line[end..]);
                    *line = new_line;
                    col = pos + replace_with.len();
                    count += 1;
                    buf.dirty = true;
                }
                None => break,
            }
        }
    }

    let mut msg = [0u8; 80];
    let prefix = b"Replaced ";
    msg[..prefix.len()].copy_from_slice(prefix);
    let mut i = prefix.len();
    i += write_num(&mut msg[i..], count as usize);
    let suffix = b" occurrences";
    msg[i..i + suffix.len()].copy_from_slice(suffix);
    i += suffix.len();
    buf.status_msg[..i].copy_from_slice(&msg[..i]);
    buf.status_len = i;
}

// ---- Go to line ----

fn goto_line(buf: &mut Buffer) {
    if let Some(input) = prompt(buf, b"Line number: ") {
        let n = parse_num(&input);
        if n > 0 && n <= buf.num_lines() {
            buf.cy = n - 1;
            buf.cx = 0;
        }
    }
}

fn parse_num(s: &[u8]) -> usize {
    let mut n = 0usize;
    for &b in s {
        if b >= b'0' && b <= b'9' {
            n = n * 10 + (b - b'0') as usize;
        }
    }
    n
}

// ---- Prompt (mini input line) ----

fn prompt(buf: &mut Buffer, msg: &[u8]) -> Option<Vec<u8>> {
    let mut input = Vec::new();

    loop {
        // Draw prompt on status line
        let mut status = [0u8; 80];
        let mlen = msg.len().min(60);
        status[..mlen].copy_from_slice(&msg[..mlen]);
        let ilen = input.len().min(80 - mlen);
        status[mlen..mlen + ilen].copy_from_slice(&input[..ilen]);
        buf.status_msg[..mlen + ilen].copy_from_slice(&status[..mlen + ilen]);
        buf.status_len = mlen + ilen;

        scroll(buf);
        render(buf);

        match read_key() {
            Key::Enter => {
                buf.status_len = 0;
                return Some(input);
            }
            Key::Ctrl(b'c') | Key::Escape => {
                buf.status_len = 0;
                return None;
            }
            Key::Backspace => { input.pop(); }
            Key::Char(c) => { input.push(c); }
            _ => {}
        }
    }
}

// ---- Help screen ----

fn show_help(buf: &mut Buffer) {
    buf.set_status(b"^O Save  ^X Exit  ^K Cut  ^U Paste  ^W Search  ^T GoTo  ^\\ Replace");
    // Just set status — a real help screen would overlay the buffer
}

// ---- Cursor movement ----

fn move_cursor(buf: &mut Buffer, key: &Key) {
    match key {
        Key::Left => {
            if buf.cx > 0 {
                buf.cx -= 1;
            } else if buf.cy > 0 {
                buf.cy -= 1;
                buf.cx = buf.lines[buf.cy].len();
            }
        }
        Key::Right => {
            let cy = buf.cy;
            if cy < buf.num_lines() {
                if buf.cx < buf.lines[cy].len() {
                    buf.cx += 1;
                } else if cy + 1 < buf.num_lines() {
                    buf.cy += 1;
                    buf.cx = 0;
                }
            }
        }
        Key::Up => {
            if buf.cy > 0 { buf.cy -= 1; }
        }
        Key::Down => {
            if buf.cy + 1 < buf.num_lines() { buf.cy += 1; }
        }
        Key::Home => { buf.cx = 0; }
        Key::End => {
            let cy = buf.cy;
            buf.cx = buf.lines[cy].len();
        }
        Key::PageUp => {
            buf.cy = buf.cy.saturating_sub(buf.screen_rows);
        }
        Key::PageDown => {
            buf.cy = (buf.cy + buf.screen_rows).min(buf.num_lines().saturating_sub(1));
        }
        _ => {}
    }
    // Snap cx to line length
    if buf.cy < buf.num_lines() {
        let len = buf.lines[buf.cy].len();
        if buf.cx > len { buf.cx = len; }
    }
}

// ---- Confirm exit ----

fn confirm_exit(buf: &mut Buffer) -> bool {
    if !buf.dirty { return true; }
    buf.set_status(b"Save modified buffer? (Y)es (N)o (C)ancel");
    scroll(buf);
    render(buf);

    loop {
        match read_key() {
            Key::Char(b'y') | Key::Char(b'Y') => {
                save_file(buf);
                return true;
            }
            Key::Char(b'n') | Key::Char(b'N') => {
                return true;
            }
            Key::Char(b'c') | Key::Char(b'C') | Key::Ctrl(b'c') | Key::Escape => {
                buf.status_len = 0;
                return false;
            }
            _ => {}
        }
    }
}

// ---- Main ----

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    syscall::set_raw_mode(true);

    let mut buf = Buffer::new();

    // Load file if given
    if argc >= 2 {
        let name = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
        buf.set_filename(name.as_bytes());
        load_file(&mut buf);
    }

    // Clear screen
    write_str(b"\x1b[2J");

    loop {
        scroll(&mut buf);
        render(&mut buf);
        buf.status_len = 0; // clear transient status after one render

        let key = read_key();

        match key {
            Key::Ctrl(b'x') => {
                if confirm_exit(&mut buf) { break; }
            }
            Key::Ctrl(b'o') => { save_file(&mut buf); }
            Key::Ctrl(b'k') => { cut_line(&mut buf); }
            Key::Ctrl(b'u') => { paste_lines(&mut buf); }
            Key::Ctrl(b'w') => { search(&mut buf); }
            Key::Ctrl(b'g') => { show_help(&mut buf); }
            Key::Ctrl(b't') => { goto_line(&mut buf); }
            Key::Ctrl(b'\\') => { replace(&mut buf); }
            Key::Ctrl(b'c') => {
                // Show cursor position
                let mut msg = [0u8; 40];
                let prefix = b"Line ";
                msg[..prefix.len()].copy_from_slice(prefix);
                let mut i = prefix.len();
                i += write_num(&mut msg[i..], buf.cy + 1);
                let mid = b", Col ";
                msg[i..i + mid.len()].copy_from_slice(mid);
                i += mid.len();
                i += write_num(&mut msg[i..], buf.cx + 1);
                buf.status_msg[..i].copy_from_slice(&msg[..i]);
                buf.status_len = i;
            }
            Key::Up | Key::Down | Key::Left | Key::Right
            | Key::Home | Key::End | Key::PageUp | Key::PageDown => {
                move_cursor(&mut buf, &key);
            }
            Key::Backspace => { delete_char(&mut buf); }
            Key::Delete => { delete_forward(&mut buf); }
            Key::Enter => { insert_newline(&mut buf); }
            Key::Tab => { insert_tab(&mut buf); }
            Key::Char(c) if c >= 0x20 => { insert_char(&mut buf, c); }
            _ => {}
        }
    }

    // Restore terminal
    syscall::set_raw_mode(false);
    write_str(b"\x1b[2J\x1b[H");

    0
}
