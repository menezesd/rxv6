//! tetris - classic falling block game for rxv6.
//!
//! Uses raw mode for input, ANSI escape codes for display,
//! uptime() for gravity timing.

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::print;

const BOARD_W: usize = 10;
const BOARD_H: usize = 20;
const HIDDEN: usize = 2; // hidden rows above visible area

// Total board height including hidden rows
const TOTAL_H: usize = BOARD_H + HIDDEN;

// Board origin on screen (1-based)
const BOARD_X: usize = 3;
const BOARD_Y: usize = 2;

// Piece definitions: 4 rotations of 4 cells each, as (row, col) offsets
// I, O, T, S, Z, L, J
const NUM_PIECES: usize = 7;

struct Piece {
    cells: [[(i8, i8); 4]; 4], // 4 rotations, each with 4 (row,col) offsets
    color: u8,                  // ANSI color code (31-37)
}

const PIECES: [Piece; NUM_PIECES] = [
    // I
    Piece {
        cells: [
            [(0,0),(0,1),(0,2),(0,3)],
            [(0,0),(1,0),(2,0),(3,0)],
            [(0,0),(0,1),(0,2),(0,3)],
            [(0,0),(1,0),(2,0),(3,0)],
        ],
        color: 36, // cyan
    },
    // O
    Piece {
        cells: [
            [(0,0),(0,1),(1,0),(1,1)],
            [(0,0),(0,1),(1,0),(1,1)],
            [(0,0),(0,1),(1,0),(1,1)],
            [(0,0),(0,1),(1,0),(1,1)],
        ],
        color: 33, // yellow
    },
    // T
    Piece {
        cells: [
            [(0,0),(0,1),(0,2),(1,1)],
            [(0,0),(1,0),(2,0),(1,1)],
            [(1,0),(1,1),(1,2),(0,1)],
            [(0,0),(1,0),(2,0),(1,-1)],
        ],
        color: 35, // magenta
    },
    // S
    Piece {
        cells: [
            [(0,1),(0,2),(1,0),(1,1)],
            [(0,0),(1,0),(1,1),(2,1)],
            [(0,1),(0,2),(1,0),(1,1)],
            [(0,0),(1,0),(1,1),(2,1)],
        ],
        color: 32, // green
    },
    // Z
    Piece {
        cells: [
            [(0,0),(0,1),(1,1),(1,2)],
            [(0,1),(1,0),(1,1),(2,0)],
            [(0,0),(0,1),(1,1),(1,2)],
            [(0,1),(1,0),(1,1),(2,0)],
        ],
        color: 31, // red
    },
    // L
    Piece {
        cells: [
            [(0,0),(0,1),(0,2),(1,0)],
            [(0,0),(1,0),(2,0),(2,1)],
            [(1,0),(1,1),(1,2),(0,2)],
            [(0,0),(0,1),(1,1),(2,1)],
        ],
        color: 33, // orange (yellow on VGA)
    },
    // J
    Piece {
        cells: [
            [(0,0),(0,1),(0,2),(1,2)],
            [(0,0),(0,1),(1,0),(2,0)],
            [(0,0),(1,0),(1,1),(1,2)],
            [(0,0),(2,0),(2,1),(0,1)], // fixed J rotation
        ],
        color: 34, // blue
    },
];

struct Game {
    board: [[u8; BOARD_W]; TOTAL_H], // 0 = empty, color code otherwise
    cur_piece: usize,
    cur_rot: usize,
    cur_row: i16,
    cur_col: i16,
    next_piece: usize,
    score: u32,
    lines: u32,
    level: u32,
    game_over: bool,
    rng: u32,
}

impl Game {
    fn new(seed: u32) -> Self {
        let mut g = Game {
            board: [[0; BOARD_W]; TOTAL_H],
            cur_piece: 0,
            cur_rot: 0,
            cur_row: 0,
            cur_col: 0,
            next_piece: 0,
            score: 0,
            lines: 0,
            level: 1,
            game_over: false,
            rng: seed,
        };
        g.next_piece = g.rand_piece();
        g.spawn();
        g
    }

    fn rand(&mut self) -> u32 {
        // xorshift32
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng
    }

    fn rand_piece(&mut self) -> usize {
        (self.rand() % NUM_PIECES as u32) as usize
    }

    fn spawn(&mut self) {
        self.cur_piece = self.next_piece;
        self.next_piece = self.rand_piece();
        self.cur_rot = 0;
        self.cur_row = 0;
        self.cur_col = (BOARD_W as i16 / 2) - 1;

        if !self.fits(self.cur_row, self.cur_col, self.cur_rot) {
            self.game_over = true;
        }
    }

    fn cells(&self, rot: usize) -> &[(i8, i8); 4] {
        &PIECES[self.cur_piece].cells[rot % 4]
    }

    fn fits(&self, row: i16, col: i16, rot: usize) -> bool {
        for &(dr, dc) in PIECES[self.cur_piece].cells[rot % 4].iter() {
            let r = row + dr as i16;
            let c = col + dc as i16;
            if c < 0 || c >= BOARD_W as i16 || r < 0 || r >= TOTAL_H as i16 {
                return false;
            }
            if self.board[r as usize][c as usize] != 0 {
                return false;
            }
        }
        true
    }

    fn lock(&mut self) {
        let color = PIECES[self.cur_piece].color;
        let cells = PIECES[self.cur_piece].cells[self.cur_rot % 4];
        for &(dr, dc) in cells.iter() {
            let r = (self.cur_row + dr as i16) as usize;
            let c = (self.cur_col + dc as i16) as usize;
            if r < TOTAL_H && c < BOARD_W {
                self.board[r][c] = color;
            }
        }
        self.clear_lines();
        self.spawn();
    }

    fn clear_lines(&mut self) {
        let mut cleared = 0u32;
        let mut dst = TOTAL_H - 1;
        let mut src = TOTAL_H - 1;

        // Copy non-full rows from bottom
        loop {
            let full = self.board[src].iter().all(|&c| c != 0);
            if full {
                cleared += 1;
            } else {
                if dst != src {
                    self.board[dst] = self.board[src];
                }
                if dst == 0 { break; }
                dst -= 1;
            }
            if src == 0 { break; }
            src -= 1;
        }

        // Clear remaining top rows
        while dst > 0 {
            // dst might have been skipped
            if dst <= src || cleared > 0 {
                self.board[dst] = [0; BOARD_W];
            }
            if dst == 0 { break; }
            dst -= 1;
        }
        if cleared > 0 {
            self.board[0] = [0; BOARD_W];
        }

        if cleared > 0 {
            self.lines += cleared;
            self.score += match cleared {
                1 => 100 * self.level,
                2 => 300 * self.level,
                3 => 500 * self.level,
                _ => 800 * self.level,
            };
            self.level = self.lines / 10 + 1;
        }
    }

    fn move_piece(&mut self, dr: i16, dc: i16) -> bool {
        if self.fits(self.cur_row + dr, self.cur_col + dc, self.cur_rot) {
            self.cur_row += dr;
            self.cur_col += dc;
            true
        } else {
            false
        }
    }

    fn rotate(&mut self) {
        let new_rot = (self.cur_rot + 1) % 4;
        if self.fits(self.cur_row, self.cur_col, new_rot) {
            self.cur_rot = new_rot;
        } else if self.fits(self.cur_row, self.cur_col - 1, new_rot) {
            self.cur_col -= 1;
            self.cur_rot = new_rot;
        } else if self.fits(self.cur_row, self.cur_col + 1, new_rot) {
            self.cur_col += 1;
            self.cur_rot = new_rot;
        }
    }

    fn hard_drop(&mut self) {
        while self.fits(self.cur_row + 1, self.cur_col, self.cur_rot) {
            self.cur_row += 1;
            self.score += 2;
        }
        self.lock();
    }

    fn drop_interval(&self) -> u32 {
        // Ticks between gravity drops (100 ticks/sec)
        let base = 50u32; // 0.5 sec at level 1
        if self.level >= base { 2 } else { base - self.level + 1 }
    }
}

// ---- Drawing ----

fn write_str(s: &[u8]) {
    syscall::write(1, s);
}

fn goto(row: usize, col: usize) {
    let mut buf = [0u8; 16];
    let n = fmt_csi(&mut buf, row, col);
    write_str(&buf[..n]);
}

// Format ESC[row;colH
fn fmt_csi(buf: &mut [u8], row: usize, col: usize) -> usize {
    buf[0] = 0x1b;
    buf[1] = b'[';
    let mut i = 2;
    i += fmt_num(&mut buf[i..], row);
    buf[i] = b';';
    i += 1;
    i += fmt_num(&mut buf[i..], col);
    buf[i] = b'H';
    i + 1
}

fn fmt_num(buf: &mut [u8], mut n: usize) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 8];
    let mut len = 0;
    while n > 0 {
        tmp[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for j in 0..len {
        buf[j] = tmp[len - 1 - j];
    }
    len
}

fn draw_board(game: &Game) {
    // Draw border and cells
    for row in HIDDEN..TOTAL_H {
        let screen_row = BOARD_Y + (row - HIDDEN);
        goto(screen_row, BOARD_X);
        write_str(b"\x1b[90m\xe2\x94\x82\x1b[0m"); // │ in dark gray... no UTF-8, use ASCII
        // Actually, let's use simple ASCII
    }

    // Redraw full board
    for row in HIDDEN..TOTAL_H {
        let screen_row = BOARD_Y + (row - HIDDEN);
        goto(screen_row, BOARD_X);
        write_str(b"\x1b[90m|\x1b[0m");

        for col in 0..BOARD_W {
            let c = game.board[row][col];
            if c != 0 {
                // Draw locked cell with color
                draw_colored_block(c);
            } else {
                // Check if current piece occupies this cell
                let mut is_piece = false;
                for &(dr, dc) in game.cells(game.cur_rot).iter() {
                    let pr = game.cur_row + dr as i16;
                    let pc = game.cur_col + dc as i16;
                    if pr as usize == row && pc as usize == col {
                        draw_colored_block(PIECES[game.cur_piece].color);
                        is_piece = true;
                        break;
                    }
                }
                if !is_piece {
                    write_str(b"  ");
                }
            }
        }
        write_str(b"\x1b[90m|\x1b[0m");
    }

    // Bottom border
    goto(BOARD_Y + BOARD_H, BOARD_X);
    write_str(b"\x1b[90m+");
    for _ in 0..BOARD_W {
        write_str(b"--");
    }
    write_str(b"+\x1b[0m");

    // Score panel
    let panel_x = BOARD_X + BOARD_W * 2 + 4;
    goto(BOARD_Y, panel_x);
    write_str(b"\x1b[1mTETRIS\x1b[0m");

    goto(BOARD_Y + 2, panel_x);
    write_str(b"Score: ");
    print_num(game.score);
    write_str(b"   ");

    goto(BOARD_Y + 3, panel_x);
    write_str(b"Lines: ");
    print_num(game.lines);
    write_str(b"   ");

    goto(BOARD_Y + 4, panel_x);
    write_str(b"Level: ");
    print_num(game.level);
    write_str(b"   ");

    // Next piece preview
    goto(BOARD_Y + 6, panel_x);
    write_str(b"Next:");
    for r in 0..2 {
        goto(BOARD_Y + 7 + r, panel_x);
        write_str(b"    "); // clear previous
        goto(BOARD_Y + 7 + r, panel_x);
        for c in 0..4 {
            let mut drawn = false;
            for &(dr, dc) in PIECES[game.next_piece].cells[0].iter() {
                if dr as usize == r && dc as usize == c {
                    draw_colored_block(PIECES[game.next_piece].color);
                    drawn = true;
                    break;
                }
            }
            if !drawn {
                write_str(b"  ");
            }
        }
    }

    // Controls
    goto(BOARD_Y + 11, panel_x);
    write_str(b"\x1b[90mArrows: Move\x1b[0m");
    goto(BOARD_Y + 12, panel_x);
    write_str(b"\x1b[90mUp:     Rotate\x1b[0m");
    goto(BOARD_Y + 13, panel_x);
    write_str(b"\x1b[90mSpace:  Drop\x1b[0m");
    goto(BOARD_Y + 14, panel_x);
    write_str(b"\x1b[90mq:      Quit\x1b[0m");
}

fn draw_colored_block(color: u8) {
    let mut buf = [0u8; 16];
    buf[0] = 0x1b;
    buf[1] = b'[';
    let mut i = 2;
    i += fmt_num(&mut buf[i..], color as usize);
    buf[i] = b'm';
    i += 1;
    buf[i] = b'[';
    buf[i + 1] = b']';
    i += 2;
    buf[i] = 0x1b;
    buf[i + 1] = b'[';
    buf[i + 2] = b'0';
    buf[i + 3] = b'm';
    i += 4;
    write_str(&buf[..i]);
}

fn print_num(n: u32) {
    let mut buf = [0u8; 12];
    let len = fmt_num(&mut buf, n as usize);
    write_str(&buf[..len]);
}

fn draw_initial() {
    // Clear screen
    write_str(b"\x1b[2J\x1b[H");

    // Top border
    goto(BOARD_Y - 1, BOARD_X);
    write_str(b"\x1b[90m+");
    for _ in 0..BOARD_W {
        write_str(b"--");
    }
    write_str(b"+\x1b[0m");
}

// ---- Input ----

#[derive(PartialEq)]
enum Key {
    Left,
    Right,
    Down,
    Up,
    Space,
    Quit,
    None,
}

fn read_key() -> Key {
    if !syscall::input_ready() {
        return Key::None;
    }

    let mut c = [0u8; 1];
    let n = syscall::read(0, &mut c);
    if n <= 0 { return Key::Quit; }

    match c[0] {
        b'q' | b'Q' | 0x03 => Key::Quit,   // q or Ctrl-C
        b' ' => Key::Space,
        b'a' | b'A' => Key::Left,  // WASD alternative
        b'd' | b'D' => Key::Right,
        b's' | b'S' => Key::Down,
        b'w' | b'W' => Key::Up,
        0x1b => {
            // Escape sequence
            if !syscall::input_ready() { return Key::Quit; } // bare ESC = quit
            let mut seq = [0u8; 1];
            syscall::read(0, &mut seq);
            if seq[0] != b'[' { return Key::None; }
            if !syscall::input_ready() { return Key::None; }
            syscall::read(0, &mut seq);
            match seq[0] {
                b'A' => Key::Up,
                b'B' => Key::Down,
                b'C' => Key::Right,
                b'D' => Key::Left,
                _ => {
                    // Consume trailing bytes of longer sequences
                    if seq[0] >= b'0' && seq[0] <= b'9' {
                        while syscall::input_ready() {
                            let mut t = [0u8; 1];
                            syscall::read(0, &mut t);
                            if t[0] == b'~' { break; }
                        }
                    }
                    Key::None
                }
            }
        }
        _ => Key::None,
    }
}

// ---- Main ----

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    // Hide cursor and enter raw mode
    write_str(b"\x1b[?25l");
    syscall::set_raw_mode(true);

    let seed = syscall::uptime() as u32;
    let mut game = Game::new(if seed == 0 { 42 } else { seed });

    draw_initial();
    draw_board(&game);

    let mut last_drop = syscall::uptime() as u32;

    loop {
        if game.game_over {
            break;
        }

        // Handle input
        let key = read_key();
        let mut needs_redraw = key != Key::None;

        match key {
            Key::Quit => break,
            Key::Left => { game.move_piece(0, -1); }
            Key::Right => { game.move_piece(0, 1); }
            Key::Down => {
                if !game.move_piece(1, 0) {
                    game.lock();
                }
                last_drop = syscall::uptime() as u32;
            }
            Key::Up => { game.rotate(); }
            Key::Space => {
                game.hard_drop();
                last_drop = syscall::uptime() as u32;
            }
            Key::None => { needs_redraw = false; }
        }

        // Gravity
        let now = syscall::uptime() as u32;
        if now.wrapping_sub(last_drop) >= game.drop_interval() {
            if !game.move_piece(1, 0) {
                game.lock();
            }
            last_drop = now;
            needs_redraw = true;
        }

        if needs_redraw {
            draw_board(&game);
        }

        // Sleep ~10ms to avoid burning CPU
        syscall::sleep(1);
    }

    // Game over screen
    let panel_x = BOARD_X + BOARD_W * 2 + 4;
    goto(BOARD_Y + 16, panel_x);
    write_str(b"\x1b[1;31mGAME OVER\x1b[0m");
    goto(BOARD_Y + 17, panel_x);
    write_str(b"Score: ");
    print_num(game.score);

    // Wait for keypress
    goto(BOARD_Y + 19, panel_x);
    write_str(b"Press any key...");
    let mut c = [0u8; 1];
    syscall::read(0, &mut c);

    // Restore terminal
    syscall::set_raw_mode(false);
    write_str(b"\x1b[?25h"); // show cursor
    write_str(b"\x1b[2J\x1b[H"); // clear screen

    0
}
