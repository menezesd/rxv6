//! chess - a chess engine for rxv6.
//!
//! Features:
//!   - 8x8 mailbox board representation
//!   - Full legal move generation (castling, en passant, promotion)
//!   - Alpha-beta search with quiescence search
//!   - Piece-square table evaluation
//!   - Iterative deepening (default depth 4)
//!   - Text UI with algebraic coordinate notation (e.g. e2e4)
//!
//! Commands:
//!   e2e4    - make a move (from-square to-square)
//!   e7e8q   - pawn promotion (append piece letter)
//!   go      - force computer to move (switch sides)
//!   new     - start new game
//!   board   - print the board
//!   quit    - exit

#![no_std]
#![no_main]

use rxv6_user::syscall;
use rxv6_user::{print, println};

// ---- Piece encoding --------------------------------------------------------

const EMPTY: u8 = 0;
const W_PAWN: u8 = 1;
const W_KNIGHT: u8 = 2;
const W_BISHOP: u8 = 3;
const W_ROOK: u8 = 4;
const W_QUEEN: u8 = 5;
const W_KING: u8 = 6;
const B_PAWN: u8 = 9;
const B_KNIGHT: u8 = 10;
const B_BISHOP: u8 = 11;
const B_ROOK: u8 = 12;
const B_QUEEN: u8 = 13;
const B_KING: u8 = 14;

const WHITE: u8 = 0;
const BLACK: u8 = 8;

fn color(p: u8) -> u8 { p & 8 }
fn kind(p: u8) -> u8 { p & 7 }
fn is_white(p: u8) -> bool { p != EMPTY && p < 8 }
fn is_black(p: u8) -> bool { p >= 9 }
fn is_piece(p: u8) -> bool { p != EMPTY }

fn piece_char(p: u8) -> u8 {
    match p {
        W_PAWN => b'P', W_KNIGHT => b'N', W_BISHOP => b'B',
        W_ROOK => b'R', W_QUEEN => b'Q', W_KING => b'K',
        B_PAWN => b'p', B_KNIGHT => b'n', B_BISHOP => b'b',
        B_ROOK => b'r', B_QUEEN => b'q', B_KING => b'k',
        _ => b'.',
    }
}

// ---- Board -----------------------------------------------------------------

// Square index: 0=a1, 1=b1, ..., 7=h1, 8=a2, ..., 63=h8
fn sq(file: i32, rank: i32) -> usize { (rank * 8 + file) as usize }
fn file_of(s: usize) -> i32 { (s % 8) as i32 }
fn rank_of(s: usize) -> i32 { (s / 8) as i32 }
fn on_board(f: i32, r: i32) -> bool { f >= 0 && f < 8 && r >= 0 && r < 8 }

#[derive(Clone, Copy)]
struct Board {
    piece: [u8; 64],
    side: u8,           // WHITE or BLACK
    castle: u8,         // bits: 1=WK, 2=WQ, 4=BK, 8=BQ
    ep: i8,             // en passant file (0-7) or -1
    half_moves: u16,    // for 50-move rule
    king_sq: [usize; 2], // [white_king, black_king]
}

impl Board {
    fn new() -> Self {
        let mut b = Board {
            piece: [EMPTY; 64],
            side: WHITE,
            castle: 0xF,
            ep: -1,
            half_moves: 0,
            king_sq: [4, 60],
        };
        // Pawns
        for f in 0..8 {
            b.piece[sq(f, 1)] = W_PAWN;
            b.piece[sq(f, 6)] = B_PAWN;
        }
        // Pieces
        let back = [W_ROOK, W_KNIGHT, W_BISHOP, W_QUEEN, W_KING, W_BISHOP, W_KNIGHT, W_ROOK];
        for f in 0..8 {
            b.piece[sq(f, 0)] = back[f as usize];
            b.piece[sq(f, 7)] = back[f as usize] | BLACK;
        }
        b
    }
}

// ---- Move encoding ---------------------------------------------------------
// 16-bit: bits 0-5 = from, 6-11 = to, 12-15 = flags
// Flags: 0=normal, 1=double pawn push, 2=king castle, 3=queen castle,
//        4=capture, 5=ep capture, 8-11=promotion (8+piece: N=0,B=1,R=2,Q=3)

const FLAG_DOUBLE: u16 = 1;
const FLAG_KS_CASTLE: u16 = 2;
const FLAG_QS_CASTLE: u16 = 3;
const FLAG_CAPTURE: u16 = 4;
const FLAG_EP: u16 = 5;
const FLAG_PROMO_N: u16 = 8;
const FLAG_PROMO_B: u16 = 9;
const FLAG_PROMO_R: u16 = 10;
const FLAG_PROMO_Q: u16 = 11;
const FLAG_PROMO_CAP_N: u16 = 12;
const FLAG_PROMO_CAP_B: u16 = 13;
const FLAG_PROMO_CAP_R: u16 = 14;
const FLAG_PROMO_CAP_Q: u16 = 15;

fn mv_new(from: usize, to: usize, flag: u16) -> u16 {
    (from as u16) | ((to as u16) << 6) | (flag << 12)
}
fn mv_from(m: u16) -> usize { (m & 0x3F) as usize }
fn mv_to(m: u16) -> usize { ((m >> 6) & 0x3F) as usize }
fn mv_flag(m: u16) -> u16 { m >> 12 }
fn is_capture(m: u16) -> bool { let f = mv_flag(m); f == FLAG_CAPTURE || f == FLAG_EP || f >= 12 }
fn is_promo(m: u16) -> bool { mv_flag(m) >= 8 }

fn promo_piece(m: u16, side: u8) -> u8 {
    let base = match mv_flag(m) & 3 {
        0 => 2, // knight
        1 => 3, // bishop
        2 => 4, // rook
        _ => 5, // queen
    };
    base | side
}

// ---- Move generation -------------------------------------------------------

const MAX_MOVES: usize = 256;

struct MoveList {
    moves: [u16; MAX_MOVES],
    count: usize,
}

impl MoveList {
    fn new() -> Self { MoveList { moves: [0; MAX_MOVES], count: 0 } }
    fn push(&mut self, m: u16) {
        if self.count < MAX_MOVES { self.moves[self.count] = m; self.count += 1; }
    }
}

const KNIGHT_DIRS: [(i32, i32); 8] = [
    (-2,-1),(-2,1),(-1,-2),(-1,2),(1,-2),(1,2),(2,-1),(2,1)
];
const BISHOP_DIRS: [(i32, i32); 4] = [(-1,-1),(-1,1),(1,-1),(1,1)];
const ROOK_DIRS: [(i32, i32); 4] = [(-1,0),(1,0),(0,-1),(0,1)];

fn gen_moves(b: &Board) -> MoveList {
    let mut ml = MoveList::new();
    let side = b.side;
    let opp = side ^ 8;

    for s in 0..64 {
        let p = b.piece[s];
        if p == EMPTY || color(p) != side { continue; }
        let f = file_of(s);
        let r = rank_of(s);

        match kind(p) {
            1 => gen_pawn_moves(b, s, f, r, side, opp, &mut ml),
            2 => gen_step_moves(b, s, f, r, side, &KNIGHT_DIRS, &mut ml),
            3 => gen_slide_moves(b, s, f, r, side, &BISHOP_DIRS, &mut ml),
            4 => gen_slide_moves(b, s, f, r, side, &ROOK_DIRS, &mut ml),
            5 => {
                gen_slide_moves(b, s, f, r, side, &BISHOP_DIRS, &mut ml);
                gen_slide_moves(b, s, f, r, side, &ROOK_DIRS, &mut ml);
            }
            6 => {
                let king_dirs: [(i32,i32); 8] = [
                    (-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,-1),(1,0),(1,1)
                ];
                gen_step_moves(b, s, f, r, side, &king_dirs, &mut ml);
                gen_castle_moves(b, s, side, &mut ml);
            }
            _ => {}
        }
    }
    ml
}

fn gen_pawn_moves(b: &Board, s: usize, f: i32, r: i32, side: u8, _opp: u8, ml: &mut MoveList) {
    let dir: i32 = if side == WHITE { 1 } else { -1 };
    let start_rank = if side == WHITE { 1 } else { 6 };
    let promo_rank = if side == WHITE { 7 } else { 0 };

    // Forward one
    let nr = r + dir;
    if on_board(f, nr) {
        let to = sq(f, nr);
        if b.piece[to] == EMPTY {
            if nr == promo_rank {
                ml.push(mv_new(s, to, FLAG_PROMO_N));
                ml.push(mv_new(s, to, FLAG_PROMO_B));
                ml.push(mv_new(s, to, FLAG_PROMO_R));
                ml.push(mv_new(s, to, FLAG_PROMO_Q));
            } else {
                ml.push(mv_new(s, to, 0));
                // Double push
                if r == start_rank {
                    let nr2 = r + dir * 2;
                    let to2 = sq(f, nr2);
                    if b.piece[to2] == EMPTY {
                        ml.push(mv_new(s, to2, FLAG_DOUBLE));
                    }
                }
            }
        }
    }

    // Captures
    for df in [-1i32, 1] {
        let nf = f + df;
        if !on_board(nf, nr) { continue; }
        let to = sq(nf, nr);
        let target = b.piece[to];

        // Normal capture
        if is_piece(target) && color(target) != side {
            if nr == promo_rank {
                ml.push(mv_new(s, to, FLAG_PROMO_CAP_N));
                ml.push(mv_new(s, to, FLAG_PROMO_CAP_B));
                ml.push(mv_new(s, to, FLAG_PROMO_CAP_R));
                ml.push(mv_new(s, to, FLAG_PROMO_CAP_Q));
            } else {
                ml.push(mv_new(s, to, FLAG_CAPTURE));
            }
        }

        // En passant
        if b.ep >= 0 && nf == b.ep as i32 {
            let ep_rank = if side == WHITE { 5 } else { 2 };
            if nr == ep_rank {
                ml.push(mv_new(s, to, FLAG_EP));
            }
        }
    }
}

fn gen_step_moves(b: &Board, s: usize, f: i32, r: i32, side: u8, dirs: &[(i32,i32)], ml: &mut MoveList) {
    for &(df, dr) in dirs {
        let nf = f + df;
        let nr = r + dr;
        if !on_board(nf, nr) { continue; }
        let to = sq(nf, nr);
        let target = b.piece[to];
        if target == EMPTY {
            ml.push(mv_new(s, to, 0));
        } else if color(target) != side {
            ml.push(mv_new(s, to, FLAG_CAPTURE));
        }
    }
}

fn gen_slide_moves(b: &Board, s: usize, f: i32, r: i32, side: u8, dirs: &[(i32,i32)], ml: &mut MoveList) {
    for &(df, dr) in dirs {
        let mut nf = f + df;
        let mut nr = r + dr;
        while on_board(nf, nr) {
            let to = sq(nf, nr);
            let target = b.piece[to];
            if target == EMPTY {
                ml.push(mv_new(s, to, 0));
            } else {
                if color(target) != side {
                    ml.push(mv_new(s, to, FLAG_CAPTURE));
                }
                break;
            }
            nf += df;
            nr += dr;
        }
    }
}

fn gen_castle_moves(b: &Board, king_sq: usize, side: u8, ml: &mut MoveList) {
    if side == WHITE {
        // King-side: e1-g1, f1+g1 must be empty, e1+f1+g1 not attacked
        if (b.castle & 1) != 0 && king_sq == 4
            && b.piece[5] == EMPTY && b.piece[6] == EMPTY
            && !sq_attacked(b, 4, BLACK) && !sq_attacked(b, 5, BLACK) && !sq_attacked(b, 6, BLACK)
        {
            ml.push(mv_new(4, 6, FLAG_KS_CASTLE));
        }
        // Queen-side: e1-c1
        if (b.castle & 2) != 0 && king_sq == 4
            && b.piece[3] == EMPTY && b.piece[2] == EMPTY && b.piece[1] == EMPTY
            && !sq_attacked(b, 4, BLACK) && !sq_attacked(b, 3, BLACK) && !sq_attacked(b, 2, BLACK)
        {
            ml.push(mv_new(4, 2, FLAG_QS_CASTLE));
        }
    } else {
        if (b.castle & 4) != 0 && king_sq == 60
            && b.piece[61] == EMPTY && b.piece[62] == EMPTY
            && !sq_attacked(b, 60, WHITE) && !sq_attacked(b, 61, WHITE) && !sq_attacked(b, 62, WHITE)
        {
            ml.push(mv_new(60, 62, FLAG_KS_CASTLE));
        }
        if (b.castle & 8) != 0 && king_sq == 60
            && b.piece[59] == EMPTY && b.piece[58] == EMPTY && b.piece[57] == EMPTY
            && !sq_attacked(b, 60, WHITE) && !sq_attacked(b, 59, WHITE) && !sq_attacked(b, 58, WHITE)
        {
            ml.push(mv_new(60, 58, FLAG_QS_CASTLE));
        }
    }
}

// ---- Attack detection ------------------------------------------------------

fn sq_attacked(b: &Board, s: usize, by_side: u8) -> bool {
    let f = file_of(s);
    let r = rank_of(s);

    // Pawn attacks
    let pdir: i32 = if by_side == WHITE { -1 } else { 1 }; // direction from target's perspective
    for df in [-1i32, 1] {
        let pf = f + df;
        let pr = r + pdir;
        if on_board(pf, pr) {
            let attacker = b.piece[sq(pf, pr)];
            if kind(attacker) == 1 && color(attacker) == by_side { return true; }
        }
    }

    // Knight
    for &(df, dr) in &KNIGHT_DIRS {
        let nf = f + df;
        let nr = r + dr;
        if on_board(nf, nr) {
            let attacker = b.piece[sq(nf, nr)];
            if kind(attacker) == 2 && color(attacker) == by_side { return true; }
        }
    }

    // King
    for df in -1..=1i32 {
        for dr in -1..=1i32 {
            if df == 0 && dr == 0 { continue; }
            let nf = f + df;
            let nr = r + dr;
            if on_board(nf, nr) {
                let attacker = b.piece[sq(nf, nr)];
                if kind(attacker) == 6 && color(attacker) == by_side { return true; }
            }
        }
    }

    // Sliders: bishop/queen on diagonals
    for &(df, dr) in &BISHOP_DIRS {
        let mut nf = f + df;
        let mut nr = r + dr;
        while on_board(nf, nr) {
            let p = b.piece[sq(nf, nr)];
            if p != EMPTY {
                if color(p) == by_side && (kind(p) == 3 || kind(p) == 5) { return true; }
                break;
            }
            nf += df;
            nr += dr;
        }
    }

    // Sliders: rook/queen on files/ranks
    for &(df, dr) in &ROOK_DIRS {
        let mut nf = f + df;
        let mut nr = r + dr;
        while on_board(nf, nr) {
            let p = b.piece[sq(nf, nr)];
            if p != EMPTY {
                if color(p) == by_side && (kind(p) == 4 || kind(p) == 5) { return true; }
                break;
            }
            nf += df;
            nr += dr;
        }
    }

    false
}

fn in_check(b: &Board, side: u8) -> bool {
    let idx = (side >> 3) as usize; // 0 for white, 1 for black
    sq_attacked(b, b.king_sq[idx], side ^ 8)
}

// ---- Make / unmake move ----------------------------------------------------

#[derive(Clone, Copy)]
struct Undo {
    captured: u8,
    castle: u8,
    ep: i8,
    half_moves: u16,
    king_sq: [usize; 2],
}

fn make_move(b: &mut Board, m: u16) -> Undo {
    let undo = Undo {
        captured: EMPTY,
        castle: b.castle,
        ep: b.ep,
        half_moves: b.half_moves,
        king_sq: b.king_sq,
    };

    let from = mv_from(m);
    let to = mv_to(m);
    let flag = mv_flag(m);
    let piece = b.piece[from];
    let mut captured = b.piece[to];

    b.ep = -1;
    b.half_moves += 1;

    // Reset 50-move counter on pawn move or capture
    if kind(piece) == 1 || captured != EMPTY { b.half_moves = 0; }

    match flag {
        FLAG_DOUBLE => {
            b.piece[to] = piece;
            b.piece[from] = EMPTY;
            b.ep = file_of(from) as i8;
        }
        FLAG_EP => {
            let cap_sq = if b.side == WHITE { to - 8 } else { to + 8 };
            captured = b.piece[cap_sq];
            b.piece[cap_sq] = EMPTY;
            b.piece[to] = piece;
            b.piece[from] = EMPTY;
        }
        FLAG_KS_CASTLE => {
            b.piece[to] = piece;
            b.piece[from] = EMPTY;
            // Move rook
            if b.side == WHITE {
                b.piece[5] = W_ROOK;
                b.piece[7] = EMPTY;
            } else {
                b.piece[61] = B_ROOK;
                b.piece[63] = EMPTY;
            }
        }
        FLAG_QS_CASTLE => {
            b.piece[to] = piece;
            b.piece[from] = EMPTY;
            if b.side == WHITE {
                b.piece[3] = W_ROOK;
                b.piece[0] = EMPTY;
            } else {
                b.piece[59] = B_ROOK;
                b.piece[56] = EMPTY;
            }
        }
        f if f >= 8 => {
            // Promotion
            b.piece[to] = promo_piece(m, b.side);
            b.piece[from] = EMPTY;
        }
        _ => {
            // Normal or capture
            b.piece[to] = piece;
            b.piece[from] = EMPTY;
        }
    }

    // Update king position
    if kind(piece) == 6 {
        b.king_sq[(b.side >> 3) as usize] = to;
    }

    // Update castling rights
    if from == 0 || to == 0 { b.castle &= !2; }   // a1 rook
    if from == 7 || to == 7 { b.castle &= !1; }   // h1 rook
    if from == 56 || to == 56 { b.castle &= !8; }  // a8 rook
    if from == 63 || to == 63 { b.castle &= !4; }  // h8 rook
    if from == 4 { b.castle &= !3; }                // white king
    if from == 60 { b.castle &= !12; }              // black king

    b.side ^= 8;

    Undo { captured, ..undo }
}

fn unmake_move(b: &mut Board, m: u16, undo: &Undo) {
    b.side ^= 8;
    let from = mv_from(m);
    let to = mv_to(m);
    let flag = mv_flag(m);

    match flag {
        FLAG_EP => {
            let cap_sq = if b.side == WHITE { to - 8 } else { to + 8 };
            b.piece[from] = b.piece[to];
            b.piece[to] = EMPTY;
            b.piece[cap_sq] = undo.captured;
        }
        FLAG_KS_CASTLE => {
            b.piece[from] = b.piece[to];
            b.piece[to] = EMPTY;
            if b.side == WHITE {
                b.piece[7] = W_ROOK;
                b.piece[5] = EMPTY;
            } else {
                b.piece[63] = B_ROOK;
                b.piece[61] = EMPTY;
            }
        }
        FLAG_QS_CASTLE => {
            b.piece[from] = b.piece[to];
            b.piece[to] = EMPTY;
            if b.side == WHITE {
                b.piece[0] = W_ROOK;
                b.piece[3] = EMPTY;
            } else {
                b.piece[56] = B_ROOK;
                b.piece[59] = EMPTY;
            }
        }
        f if f >= 8 => {
            // Undo promotion: restore pawn
            b.piece[from] = 1 | b.side; // pawn of our color
            b.piece[to] = undo.captured;
        }
        _ => {
            b.piece[from] = b.piece[to];
            b.piece[to] = undo.captured;
        }
    }

    b.castle = undo.castle;
    b.ep = undo.ep;
    b.half_moves = undo.half_moves;
    b.king_sq = undo.king_sq;
}

// ---- Legal move filter -----------------------------------------------------

fn gen_legal_moves(b: &Board) -> MoveList {
    let pseudo = gen_moves(b);
    let mut legal = MoveList::new();
    let mut bc = *b;

    for i in 0..pseudo.count {
        let m = pseudo.moves[i];
        let undo = make_move(&mut bc, m);
        // After making the move, it's opponent's turn.
        // Check if OUR king is in check (illegal).
        if !in_check(&bc, bc.side ^ 8) {
            legal.push(m);
        }
        unmake_move(&mut bc, m, &undo);
    }
    legal
}

// ---- Evaluation ------------------------------------------------------------

const PIECE_VAL: [i32; 7] = [0, 100, 320, 330, 500, 900, 20000];

// Piece-square tables (from white's perspective, a1=index 0)
// Pawns: encourage center control and advancement
static PST_PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10,-20,-20, 10, 10,  5,
     5, -5,-10,  0,  0,-10, -5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5,  5, 10, 25, 25, 10,  5,  5,
    10, 10, 20, 30, 30, 20, 10, 10,
    50, 50, 50, 50, 50, 50, 50, 50,
     0,  0,  0,  0,  0,  0,  0,  0,
];

static PST_KNIGHT: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50,
];

static PST_BISHOP: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -20,-10,-10,-10,-10,-10,-10,-20,
];

static PST_ROOK: [i32; 64] = [
     0,  0,  0,  5,  5,  0,  0,  0,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     5, 10, 10, 10, 10, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];

static PST_QUEEN: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -10,  5,  5,  5,  5,  5,  0,-10,
      0,  0,  5,  5,  5,  5,  0, -5,
     -5,  0,  5,  5,  5,  5,  0, -5,
    -10,  0,  5,  5,  5,  5,  0,-10,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20,
];

static PST_KING_MID: [i32; 64] = [
     20, 30, 10,  0,  0, 10, 30, 20,
     20, 20,  0,  0,  0,  0, 20, 20,
    -10,-20,-20,-20,-20,-20,-20,-10,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
];

fn pst_val(kind: u8, sq: usize) -> i32 {
    match kind {
        1 => PST_PAWN[sq],
        2 => PST_KNIGHT[sq],
        3 => PST_BISHOP[sq],
        4 => PST_ROOK[sq],
        5 => PST_QUEEN[sq],
        6 => PST_KING_MID[sq],
        _ => 0,
    }
}

fn mirror_sq(s: usize) -> usize {
    // Flip rank: rank 0 <-> rank 7
    let f = s % 8;
    let r = 7 - s / 8;
    r * 8 + f
}

fn evaluate(b: &Board) -> i32 {
    let mut score: i32 = 0;
    for s in 0..64 {
        let p = b.piece[s];
        if p == EMPTY { continue; }
        let k = kind(p);
        let v = PIECE_VAL[k as usize];
        if is_white(p) {
            score += v + pst_val(k, s);
        } else {
            score -= v + pst_val(k, mirror_sq(s));
        }
    }
    // Return score relative to side to move
    if b.side == WHITE { score } else { -score }
}

// ---- Search ----------------------------------------------------------------

const INF: i32 = 30000;
const MATE: i32 = 29000;

struct SearchState {
    nodes: u32,
    best_move: u16,
}

fn quiesce(b: &mut Board, mut alpha: i32, beta: i32, ss: &mut SearchState) -> i32 {
    ss.nodes += 1;
    let stand_pat = evaluate(b);
    if stand_pat >= beta { return beta; }
    if stand_pat > alpha { alpha = stand_pat; }

    let moves = gen_moves(b);
    for i in 0..moves.count {
        let m = moves.moves[i];
        if !is_capture(m) { continue; }

        let undo = make_move(b, m);
        if in_check(b, b.side ^ 8) {
            unmake_move(b, m, &undo);
            continue;
        }
        let score = -quiesce(b, -beta, -alpha, ss);
        unmake_move(b, m, &undo);

        if score >= beta { return beta; }
        if score > alpha { alpha = score; }
    }
    alpha
}

fn alpha_beta(b: &mut Board, depth: i32, mut alpha: i32, beta: i32, ss: &mut SearchState, ply: i32) -> i32 {
    if depth <= 0 { return quiesce(b, alpha, beta, ss); }

    ss.nodes += 1;
    let moves = gen_legal_moves(b);

    if moves.count == 0 {
        if in_check(b, b.side) {
            return -(MATE - ply); // checkmate
        }
        return 0; // stalemate
    }

    // Move ordering: captures first, then promotions, then quiet
    let mut ordered = [0u16; MAX_MOVES];
    let mut oc = 0;
    // Captures + promotions first
    for i in 0..moves.count {
        let m = moves.moves[i];
        if is_capture(m) || is_promo(m) { ordered[oc] = m; oc += 1; }
    }
    // Then quiet moves
    for i in 0..moves.count {
        let m = moves.moves[i];
        if !is_capture(m) && !is_promo(m) { ordered[oc] = m; oc += 1; }
    }

    for i in 0..oc {
        let m = ordered[i];
        let undo = make_move(b, m);
        let score = -alpha_beta(b, depth - 1, -beta, -alpha, ss, ply + 1);
        unmake_move(b, m, &undo);

        if score >= beta { return beta; }
        if score > alpha {
            alpha = score;
            if ply == 0 { ss.best_move = m; }
        }
    }
    alpha
}

fn search(b: &mut Board, max_depth: i32) -> u16 {
    let mut ss = SearchState { nodes: 0, best_move: 0 };
    let mut best = 0u16;

    // Iterative deepening
    for depth in 1..=max_depth {
        ss.best_move = 0;
        let score = alpha_beta(b, depth, -INF, INF, &mut ss, 0);
        if ss.best_move != 0 { best = ss.best_move; }
        let from = mv_from(best);
        let to = mv_to(best);
        print!("depth {} score {} nodes {} pv ",
            depth, score, ss.nodes);
        print_sq(from);
        print_sq(to);
        if is_promo(best) {
            let ch = match mv_flag(best) & 3 { 0=>b'n', 1=>b'b', 2=>b'r', _=>b'q' };
            print!("{}", ch as char);
        }
        println!();
    }
    best
}

// ---- Display ---------------------------------------------------------------

fn print_board(b: &Board) {
    // Uppercase pieces for both sides; black pieces marked with *
    // Dark squares shown with .:. when empty
    println!();
    let sep = "    +---+---+---+---+---+---+---+---+";
    println!("{}", sep);
    for r in (0..8).rev() {
        print!(" {}  |", (b'1' + r as u8) as char);
        for f in 0..8 {
            let p = b.piece[sq(f, r)];
            if p == EMPTY {
                print!("   |");
            } else {
                let ch = match kind(p) {
                    1 => 'P', 2 => 'N', 3 => 'B',
                    4 => 'R', 5 => 'Q', 6 => 'K', _ => '?',
                };
                if is_black(p) {
                    print!("*{} |", ch);
                } else {
                    print!(" {} |", ch);
                }
            }
        }
        println!();
        println!("{}", sep);
    }
    println!("      a   b   c   d   e   f   g   h");
    println!();
    print!("  ");
    if b.side == WHITE { print!("White"); } else { print!("Black"); }
    println!(" to move    (* = black)");
}

fn print_sq(s: usize) {
    let f = (b'a' + file_of(s) as u8) as char;
    let r = (b'1' + rank_of(s) as u8) as char;
    print!("{}{}", f, r);
}

fn print_move(m: u16) {
    print_sq(mv_from(m));
    print_sq(mv_to(m));
    if is_promo(m) {
        let ch = match mv_flag(m) & 3 { 0=>b'n', 1=>b'b', 2=>b'r', _=>b'q' };
        print!("{}", ch as char);
    }
}

// ---- Input parsing ---------------------------------------------------------

fn parse_move(b: &Board, input: &[u8]) -> Option<u16> {
    let len = input.len();
    if len < 4 { return None; }

    let ff = (input[0] as i32) - b'a' as i32;
    let fr = (input[1] as i32) - b'1' as i32;
    let tf = (input[2] as i32) - b'a' as i32;
    let tr = (input[3] as i32) - b'1' as i32;

    if !on_board(ff, fr) || !on_board(tf, tr) { return None; }

    let from = sq(ff, fr);
    let to = sq(tf, tr);

    // Check for promotion suffix
    let promo_char = if len > 4 { Some(input[4]) } else { None };

    let legal = gen_legal_moves(b);
    for i in 0..legal.count {
        let m = legal.moves[i];
        if mv_from(m) == from && mv_to(m) == to {
            if is_promo(m) {
                let want = match promo_char {
                    Some(b'n') | Some(b'N') => 0,
                    Some(b'b') | Some(b'B') => 1,
                    Some(b'r') | Some(b'R') => 2,
                    Some(b'q') | Some(b'Q') | None => 3,
                    _ => 3,
                };
                if (mv_flag(m) & 3) == want { return Some(m); }
            } else {
                return Some(m);
            }
        }
    }
    None
}

// ---- Main ------------------------------------------------------------------

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

#[no_mangle]
pub extern "C" fn rust_main(_argc: i32, _argv: *const *const u8) -> i32 {
    println!("rxv6 chess");
    println!("enter moves as e2e4, or: go, new, board, quit");

    let mut board = Board::new();
    let mut computer_side: u8 = BLACK; // computer plays black by default
    let search_depth: i32 = 4;

    print_board(&board);

    let mut buf = [0u8; 64];
    loop {
        // If it's the computer's turn, search and play
        if board.side == computer_side {
            let m = search(&mut board, search_depth);
            if m == 0 {
                println!("(no legal moves)");
                computer_side = 0xFF; // stop
                continue;
            }
            print!("computer: ");
            print_move(m);
            println!();
            make_move(&mut board, m);
            print_board(&board);

            // Check game end
            let legal = gen_legal_moves(&board);
            if legal.count == 0 {
                if in_check(&board, board.side) {
                    println!("Checkmate!");
                } else {
                    println!("Stalemate!");
                }
                computer_side = 0xFF;
            } else if in_check(&board, board.side) {
                println!("Check!");
            }
            continue;
        }

        print!("> ");
        let n = getline(&mut buf);
        if n < 0 { break; }
        let len = n as usize;
        if len == 0 { continue; }
        let cmd = &buf[..len];

        if cmd == b"quit" || cmd == b"q" { break; }
        if cmd == b"new" {
            board = Board::new();
            computer_side = BLACK;
            print_board(&board);
            continue;
        }
        if cmd == b"board" {
            print_board(&board);
            continue;
        }
        if cmd == b"go" {
            computer_side = board.side;
            continue;
        }
        if cmd == b"moves" {
            let legal = gen_legal_moves(&board);
            print!("{} legal moves: ", legal.count);
            for i in 0..legal.count {
                if i > 0 { print!(" "); }
                print_move(legal.moves[i]);
            }
            println!();
            continue;
        }
        if cmd == b"help" {
            println!("commands: e2e4 (move), go, new, board, moves, quit");
            continue;
        }

        // Try to parse as a move
        match parse_move(&board, cmd) {
            Some(m) => {
                make_move(&mut board, m);
                print_board(&board);

                let legal = gen_legal_moves(&board);
                if legal.count == 0 {
                    if in_check(&board, board.side) {
                        println!("Checkmate!");
                    } else {
                        println!("Stalemate!");
                    }
                    computer_side = 0xFF;
                } else if in_check(&board, board.side) {
                    println!("Check!");
                }
            }
            None => {
                println!("illegal move or unknown command");
            }
        }
    }
    0
}
