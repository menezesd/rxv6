//! sh - Bourne-like shell for rxv6.
//!
//! Features:
//! - Variables: VAR=value, $VAR, ${VAR}, $?, $$, $#, $0-$9
//! - Quoting: "double" (expands vars), 'single' (literal), \escape
//! - Control flow: if/then/elif/else/fi, while/do/done, for/in/do/done
//! - Connectors: ; && || &
//! - Pipes and redirections: | < > >>
//! - Builtins: cd, exit, export, unset, read, test/[, set, true, false, clear, .
//! - Command history: up/down arrows (raw mode)
//! - Script execution: sh script.sh or . script

#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use rxv6_user::syscall;
use rxv6_user::{print, println};

const BUFSIZE: usize = 512;
const HISTORY_SIZE: usize = 32;

// ---- Variable table --------------------------------------------------------

struct Vars {
    entries: Vec<(String, String)>,
    exports: Vec<String>,  // names that are exported
    last_status: i32,
    script_args: Vec<String>,
}

impl Vars {
    fn new() -> Self {
        let mut v = Vars {
            entries: Vec::new(),
            exports: Vec::new(),
            last_status: 0,
            script_args: Vec::new(),
        };
        v.set("IFS", " \t\n");
        v.set("PATH", "/");
        v
    }

    fn get(&self, name: &str) -> Option<&str> {
        for (k, v) in self.entries.iter().rev() {
            if k == name { return Some(v.as_str()); }
        }
        None
    }

    fn set(&mut self, name: &str, value: &str) {
        for entry in self.entries.iter_mut() {
            if entry.0 == name {
                entry.1 = String::from(value);
                return;
            }
        }
        self.entries.push((String::from(name), String::from(value)));
    }

    fn unset(&mut self, name: &str) {
        self.entries.retain(|(k, _)| k != name);
        self.exports.retain(|k| k != name);
    }

    fn export(&mut self, name: &str) {
        if !self.exports.iter().any(|e| e == name) {
            self.exports.push(String::from(name));
        }
    }

    /// Expand a variable reference. Handles special variables.
    fn expand(&self, name: &str) -> String {
        match name {
            "?" => format_i32(self.last_status),
            "$" => format_i32(syscall::getpid()),
            "#" => format_i32(self.script_args.len().saturating_sub(1) as i32),
            "0" => self.script_args.first().map(|s| s.clone()).unwrap_or_else(|| String::from("sh")),
            _ => {
                // $1-$9
                if name.len() == 1 {
                    let c = name.as_bytes()[0];
                    if c >= b'1' && c <= b'9' {
                        let idx = (c - b'0') as usize;
                        return self.script_args.get(idx).cloned().unwrap_or_default();
                    }
                }
                self.get(name).map(String::from).unwrap_or_default()
            }
        }
    }
}

fn format_i32(n: i32) -> String {
    if n == 0 { return String::from("0"); }
    let mut buf = [0u8; 12];
    let neg = n < 0;
    let mut val = if neg { (-(n as i64)) as u32 } else { n as u32 };
    let mut i = buf.len();
    while val > 0 {
        i -= 1;
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    if neg { i -= 1; buf[i] = b'-'; }
    String::from(unsafe { core::str::from_utf8_unchecked(&buf[i..]) })
}

// ---- Tokenizer -------------------------------------------------------------

#[derive(Clone, PartialEq, Debug)]
enum Token {
    Word(String),
    Pipe,          // |
    Semi,          // ;
    Amp,           // &
    AndAnd,        // &&
    OrOr,          // ||
    Less,          // <
    Greater,       // >
    GreaterGreater, // >>
    Newline,
    LParen,        // (
    RParen,        // )
    // Keywords (identified after tokenization)
    If, Then, Elif, Else, Fi,
    While, Until, Do, Done,
    For, In,
    Case, Esac,
    Bang,          // !
}

fn is_keyword(w: &str) -> Option<Token> {
    match w {
        "if" => Some(Token::If),
        "then" => Some(Token::Then),
        "elif" => Some(Token::Elif),
        "else" => Some(Token::Else),
        "fi" => Some(Token::Fi),
        "while" => Some(Token::While),
        "until" => Some(Token::Until),
        "do" => Some(Token::Do),
        "done" => Some(Token::Done),
        "for" => Some(Token::For),
        "in" => Some(Token::In),
        "case" => Some(Token::Case),
        "esac" => Some(Token::Esac),
        "!" => Some(Token::Bang),
        _ => None,
    }
}

fn tokenize(input: &str, vars: &Vars) -> Vec<Token> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' => { i += 1; }
            b'\n' => { tokens.push(Token::Newline); i += 1; }
            b'#' => { while i < bytes.len() && bytes[i] != b'\n' { i += 1; } }
            b';' => { tokens.push(Token::Semi); i += 1; }
            b'(' => { tokens.push(Token::LParen); i += 1; }
            b')' => { tokens.push(Token::RParen); i += 1; }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    tokens.push(Token::OrOr); i += 2;
                } else {
                    tokens.push(Token::Pipe); i += 1;
                }
            }
            b'&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                    tokens.push(Token::AndAnd); i += 2;
                } else {
                    tokens.push(Token::Amp); i += 1;
                }
            }
            b'<' => { tokens.push(Token::Less); i += 1; }
            b'>' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                    tokens.push(Token::GreaterGreater); i += 2;
                } else {
                    tokens.push(Token::Greater); i += 1;
                }
            }
            _ => {
                // Word: collect until separator, handling quotes and escapes
                let word = read_word(bytes, &mut i, vars);
                if let Some(kw) = is_keyword(&word) {
                    tokens.push(kw);
                } else {
                    tokens.push(Token::Word(word));
                }
            }
        }
    }
    tokens
}

fn read_word(bytes: &[u8], i: &mut usize, vars: &Vars) -> String {
    let mut word = String::new();
    while *i < bytes.len() {
        match bytes[*i] {
            b' ' | b'\t' | b'\n' | b';' | b'|' | b'&' | b'(' | b')' | b'<' | b'>' => break,
            b'\'' => {
                // Single quote: literal until closing '
                *i += 1;
                while *i < bytes.len() && bytes[*i] != b'\'' {
                    word.push(bytes[*i] as char);
                    *i += 1;
                }
                if *i < bytes.len() { *i += 1; } // skip closing '
            }
            b'"' => {
                // Double quote: expand variables
                *i += 1;
                while *i < bytes.len() && bytes[*i] != b'"' {
                    if bytes[*i] == b'\\' && *i + 1 < bytes.len() {
                        *i += 1;
                        word.push(bytes[*i] as char);
                        *i += 1;
                    } else if bytes[*i] == b'$' {
                        expand_var(bytes, i, vars, &mut word);
                    } else {
                        word.push(bytes[*i] as char);
                        *i += 1;
                    }
                }
                if *i < bytes.len() { *i += 1; } // skip closing "
            }
            b'\\' => {
                *i += 1;
                if *i < bytes.len() {
                    word.push(bytes[*i] as char);
                    *i += 1;
                }
            }
            b'$' => {
                expand_var(bytes, i, vars, &mut word);
            }
            b'`' => {
                // Backtick command substitution
                *i += 1;
                let mut cmd = String::new();
                while *i < bytes.len() && bytes[*i] != b'`' {
                    cmd.push(bytes[*i] as char);
                    *i += 1;
                }
                if *i < bytes.len() { *i += 1; }
                let output = command_subst(&cmd, vars);
                word.push_str(&output);
            }
            c => {
                word.push(c as char);
                *i += 1;
            }
        }
    }
    word
}

fn expand_var(bytes: &[u8], i: &mut usize, vars: &Vars, word: &mut String) {
    *i += 1; // skip $
    if *i >= bytes.len() { word.push('$'); return; }

    if bytes[*i] == b'(' {
        // $(cmd) command substitution
        *i += 1;
        let mut depth = 1;
        let mut cmd = String::new();
        while *i < bytes.len() && depth > 0 {
            if bytes[*i] == b'(' { depth += 1; }
            if bytes[*i] == b')' { depth -= 1; if depth == 0 { *i += 1; break; } }
            cmd.push(bytes[*i] as char);
            *i += 1;
        }
        let output = command_subst(&cmd, vars);
        word.push_str(&output);
    } else if bytes[*i] == b'{' {
        // ${VAR}
        *i += 1;
        let mut name = String::new();
        while *i < bytes.len() && bytes[*i] != b'}' {
            name.push(bytes[*i] as char);
            *i += 1;
        }
        if *i < bytes.len() { *i += 1; }
        word.push_str(&vars.expand(&name));
    } else if bytes[*i] == b'?' || bytes[*i] == b'$' || bytes[*i] == b'#' {
        let name = String::from(bytes[*i] as char);
        *i += 1;
        word.push_str(&vars.expand(&name));
    } else if bytes[*i].is_ascii_digit() {
        let name = String::from(bytes[*i] as char);
        *i += 1;
        word.push_str(&vars.expand(&name));
    } else if bytes[*i].is_ascii_alphanumeric() || bytes[*i] == b'_' {
        let mut name = String::new();
        while *i < bytes.len() && (bytes[*i].is_ascii_alphanumeric() || bytes[*i] == b'_') {
            name.push(bytes[*i] as char);
            *i += 1;
        }
        word.push_str(&vars.expand(&name));
    } else {
        word.push('$');
    }
}

/// Execute a command and capture its stdout output.
fn command_subst(cmd: &str, _vars: &Vars) -> String {
    let mut pipefd = [0i32; 2];
    syscall::pipe(&mut pipefd);

    let pid = syscall::fork();
    if pid == 0 {
        // Child: redirect stdout to pipe, exec command
        syscall::close(1);
        syscall::dup(pipefd[1]);
        syscall::close(pipefd[0]);
        syscall::close(pipefd[1]);

        // Build argv for sh -c "cmd"
        let sh = b"sh\0";
        let c_flag = b"-c\0";
        let mut cmd_buf = [0u8; 256];
        let len = cmd.len().min(255);
        cmd_buf[..len].copy_from_slice(&cmd.as_bytes()[..len]);
        cmd_buf[len] = 0;
        let argv: [*const u8; 4] = [sh.as_ptr(), c_flag.as_ptr(), cmd_buf.as_ptr(), core::ptr::null()];
        syscall::exec(sh.as_ptr(), argv.as_ptr());
        syscall::exit(1);
    }

    // Parent: read output from pipe
    syscall::close(pipefd[1]);
    let mut result = String::new();
    let mut buf = [0u8; 128];
    loop {
        let n = syscall::read(pipefd[0], &mut buf);
        if n <= 0 { break; }
        for &b in &buf[..n as usize] {
            result.push(b as char);
        }
    }
    syscall::close(pipefd[0]);
    let mut status: i32 = 0;
    syscall::wait(&mut status);

    // Strip trailing newlines
    while result.ends_with('\n') { result.pop(); }
    result
}

// ---- Parser (recursive descent) --------------------------------------------

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

#[derive(Clone)]
enum Cmd {
    Simple {
        argv: Vec<String>,
        stdin_file: Option<String>,
        stdout_file: Option<String>,
        append: bool,
        assignments: Vec<(String, String)>,
    },
    Pipeline(Vec<Cmd>),
    And(Box<Cmd>, Box<Cmd>),
    Or(Box<Cmd>, Box<Cmd>),
    Not(Box<Cmd>),
    Sequence(Vec<Cmd>),
    Background(Box<Cmd>),
    If {
        cond: Box<Cmd>,
        then_body: Box<Cmd>,
        elifs: Vec<(Cmd, Cmd)>,
        else_body: Option<Box<Cmd>>,
    },
    While { cond: Box<Cmd>, body: Box<Cmd> },
    Until { cond: Box<Cmd>, body: Box<Cmd> },
    For { var: String, words: Vec<String>, body: Box<Cmd> },
    Subshell(Box<Cmd>),
    Empty,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &Token) -> bool {
        if self.peek() == Some(expected) {
            self.next();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while self.peek() == Some(&Token::Newline) { self.next(); }
    }

    fn parse_list(&mut self) -> Cmd {
        self.skip_newlines();
        let mut cmds = Vec::new();
        let first = self.parse_and_or();
        match first {
            Cmd::Empty => return Cmd::Empty,
            _ => cmds.push(first),
        }

        loop {
            self.skip_newlines();
            match self.peek() {
                Some(Token::Semi) => {
                    self.next();
                    self.skip_newlines();
                    let next = self.parse_and_or();
                    match next {
                        Cmd::Empty => break,
                        _ => cmds.push(next),
                    }
                }
                Some(Token::Amp) => {
                    self.next();
                    if let Some(last) = cmds.pop() {
                        cmds.push(Cmd::Background(Box::new(last)));
                    }
                    self.skip_newlines();
                    let next = self.parse_and_or();
                    match next {
                        Cmd::Empty => break,
                        _ => cmds.push(next),
                    }
                }
                Some(Token::Newline) => {
                    self.next();
                    self.skip_newlines();
                    let next = self.parse_and_or();
                    match next {
                        Cmd::Empty => break,
                        _ => cmds.push(next),
                    }
                }
                _ => break,
            }
        }

        if cmds.len() == 1 {
            cmds.pop().unwrap()
        } else if cmds.is_empty() {
            Cmd::Empty
        } else {
            Cmd::Sequence(cmds)
        }
    }

    fn parse_and_or(&mut self) -> Cmd {
        let mut left = self.parse_pipeline();
        loop {
            match self.peek() {
                Some(Token::AndAnd) => {
                    self.next();
                    self.skip_newlines();
                    let right = self.parse_pipeline();
                    left = Cmd::And(Box::new(left), Box::new(right));
                }
                Some(Token::OrOr) => {
                    self.next();
                    self.skip_newlines();
                    let right = self.parse_pipeline();
                    left = Cmd::Or(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        left
    }

    fn parse_pipeline(&mut self) -> Cmd {
        let negate = self.expect(&Token::Bang);
        let first = self.parse_command();
        let mut cmds = vec![first];

        while self.peek() == Some(&Token::Pipe) {
            self.next();
            self.skip_newlines();
            cmds.push(self.parse_command());
        }

        let cmd = if cmds.len() == 1 {
            cmds.pop().unwrap()
        } else {
            Cmd::Pipeline(cmds)
        };

        if negate { Cmd::Not(Box::new(cmd)) } else { cmd }
    }

    fn parse_command(&mut self) -> Cmd {
        self.skip_newlines();
        match self.peek() {
            Some(Token::If) => self.parse_if(),
            Some(Token::While) => self.parse_while(),
            Some(Token::Until) => self.parse_until(),
            Some(Token::For) => self.parse_for(),
            Some(Token::LParen) => {
                self.next();
                let cmd = self.parse_list();
                self.expect(&Token::RParen);
                Cmd::Subshell(Box::new(cmd))
            }
            _ => self.parse_simple(),
        }
    }

    fn parse_simple(&mut self) -> Cmd {
        let mut argv = Vec::new();
        let mut stdin_file = None;
        let mut stdout_file = None;
        let mut append = false;
        let mut assignments = Vec::new();

        loop {
            match self.peek() {
                Some(Token::Word(w)) => {
                    let w = w.clone();
                    // Check for VAR=value assignment (only before any regular args)
                    if argv.is_empty() && !assignments.is_empty() || argv.is_empty() {
                        if let Some(eq_pos) = w.find('=') {
                            let name = &w[..eq_pos];
                            if !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                                let value = &w[eq_pos + 1..];
                                assignments.push((String::from(name), String::from(value)));
                                self.next();
                                continue;
                            }
                        }
                    }
                    argv.push(w);
                    self.next();
                }
                Some(Token::Less) => {
                    self.next();
                    if let Some(Token::Word(f)) = self.next() {
                        stdin_file = Some(f);
                    }
                }
                Some(Token::Greater) => {
                    self.next();
                    if let Some(Token::Word(f)) = self.next() {
                        stdout_file = Some(f);
                        append = false;
                    }
                }
                Some(Token::GreaterGreater) => {
                    self.next();
                    if let Some(Token::Word(f)) = self.next() {
                        stdout_file = Some(f);
                        append = true;
                    }
                }
                _ => break,
            }
        }

        if argv.is_empty() && assignments.is_empty() {
            return Cmd::Empty;
        }

        Cmd::Simple { argv, stdin_file, stdout_file, append, assignments }
    }

    fn parse_if(&mut self) -> Cmd {
        self.next(); // consume 'if'
        self.skip_newlines();
        let cond = self.parse_list();
        self.skip_newlines();
        self.expect(&Token::Then);
        self.skip_newlines();
        let then_body = self.parse_list();

        let mut elifs = Vec::new();
        let mut else_body = None;

        loop {
            self.skip_newlines();
            if self.peek() == Some(&Token::Elif) {
                self.next();
                self.skip_newlines();
                let elif_cond = self.parse_list();
                self.skip_newlines();
                self.expect(&Token::Then);
                self.skip_newlines();
                let elif_body = self.parse_list();
                elifs.push((elif_cond, elif_body));
            } else if self.peek() == Some(&Token::Else) {
                self.next();
                self.skip_newlines();
                else_body = Some(Box::new(self.parse_list()));
                break;
            } else {
                break;
            }
        }
        self.skip_newlines();
        self.expect(&Token::Fi);

        Cmd::If { cond: Box::new(cond), then_body: Box::new(then_body), elifs, else_body }
    }

    fn parse_while(&mut self) -> Cmd {
        self.next(); // consume 'while'
        self.skip_newlines();
        let cond = self.parse_list();
        self.skip_newlines();
        self.expect(&Token::Do);
        self.skip_newlines();
        let body = self.parse_list();
        self.skip_newlines();
        self.expect(&Token::Done);
        Cmd::While { cond: Box::new(cond), body: Box::new(body) }
    }

    fn parse_until(&mut self) -> Cmd {
        self.next();
        self.skip_newlines();
        let cond = self.parse_list();
        self.skip_newlines();
        self.expect(&Token::Do);
        self.skip_newlines();
        let body = self.parse_list();
        self.skip_newlines();
        self.expect(&Token::Done);
        Cmd::Until { cond: Box::new(cond), body: Box::new(body) }
    }

    fn parse_for(&mut self) -> Cmd {
        self.next(); // consume 'for'
        let var = match self.next() {
            Some(Token::Word(w)) => w,
            _ => String::from("_"),
        };
        self.skip_newlines();

        let mut words = Vec::new();
        if self.expect(&Token::In) {
            loop {
                match self.peek() {
                    Some(Token::Word(w)) => {
                        words.push(w.clone());
                        self.next();
                    }
                    _ => break,
                }
            }
        }
        self.skip_newlines();
        // Accept both ; and newline before do
        if self.peek() == Some(&Token::Semi) { self.next(); }
        self.skip_newlines();
        self.expect(&Token::Do);
        self.skip_newlines();
        let body = self.parse_list();
        self.skip_newlines();
        self.expect(&Token::Done);
        Cmd::For { var, words, body: Box::new(body) }
    }
}

// ---- Executor --------------------------------------------------------------

fn exec_cmd(cmd: &Cmd, vars: &mut Vars) -> i32 {
    match cmd {
        Cmd::Empty => 0,
        Cmd::Simple { argv, stdin_file, stdout_file, append, assignments } => {
            // Pure assignment (no command)
            if argv.is_empty() {
                for (k, v) in assignments {
                    vars.set(k, v);
                }
                return 0;
            }
            exec_simple(argv, stdin_file, stdout_file, *append, assignments, vars)
        }
        Cmd::Pipeline(cmds) => exec_pipeline(cmds, vars),
        Cmd::And(left, right) => {
            let status = exec_cmd(left, vars);
            vars.last_status = status;
            if status == 0 { exec_cmd(right, vars) } else { status }
        }
        Cmd::Or(left, right) => {
            let status = exec_cmd(left, vars);
            vars.last_status = status;
            if status != 0 { exec_cmd(right, vars) } else { status }
        }
        Cmd::Not(inner) => {
            let status = exec_cmd(inner, vars);
            if status == 0 { 1 } else { 0 }
        }
        Cmd::Sequence(cmds) => {
            let mut status = 0;
            for c in cmds {
                status = exec_cmd(c, vars);
                vars.last_status = status;
            }
            status
        }
        Cmd::Background(inner) => {
            let pid = syscall::fork();
            if pid == 0 {
                exec_cmd(inner, vars);
                syscall::exit(0);
            }
            // Don't wait
            0
        }
        Cmd::If { cond, then_body, elifs, else_body } => {
            let status = exec_cmd(cond, vars);
            vars.last_status = status;
            if status == 0 {
                return exec_cmd(then_body, vars);
            }
            for (elif_cond, elif_body) in elifs {
                let s = exec_cmd(elif_cond, vars);
                vars.last_status = s;
                if s == 0 {
                    return exec_cmd(elif_body, vars);
                }
            }
            if let Some(eb) = else_body {
                exec_cmd(eb, vars)
            } else {
                0
            }
        }
        Cmd::While { cond, body } => {
            let mut status = 0;
            loop {
                let s = exec_cmd(cond, vars);
                vars.last_status = s;
                if s != 0 { break; }
                status = exec_cmd(body, vars);
                vars.last_status = status;
            }
            status
        }
        Cmd::Until { cond, body } => {
            let mut status = 0;
            loop {
                let s = exec_cmd(cond, vars);
                vars.last_status = s;
                if s == 0 { break; }
                status = exec_cmd(body, vars);
                vars.last_status = status;
            }
            status
        }
        Cmd::For { var, words, body } => {
            let mut status = 0;
            for w in words {
                vars.set(var, w);
                status = exec_cmd(body, vars);
                vars.last_status = status;
            }
            status
        }
        Cmd::Subshell(inner) => {
            let pid = syscall::fork();
            if pid == 0 {
                let s = exec_cmd(inner, vars);
                syscall::exit(s);
            }
            let mut status: i32 = 0;
            syscall::wait(&mut status);
            status
        }
    }
}

fn exec_simple(argv: &[String], stdin_file: &Option<String>, stdout_file: &Option<String>,
               append: bool, assignments: &[(String, String)], vars: &mut Vars) -> i32 {
    let cmd = &argv[0];

    // Built-in: cd
    if cmd == "cd" {
        let dir = argv.get(1).map(|s| s.as_str()).unwrap_or("/");
        let mut path_buf = [0u8; 128];
        let len = dir.len().min(127);
        path_buf[..len].copy_from_slice(&dir.as_bytes()[..len]);
        if syscall::chdir(path_buf.as_ptr()) < 0 {
            println!("cd: cannot cd {}", dir);
            return 1;
        }
        return 0;
    }

    // Built-in: exit
    if cmd == "exit" {
        let code = argv.get(1).map(|s| parse_int(s)).unwrap_or(vars.last_status);
        syscall::exit(code);
    }

    // Built-in: export
    if cmd == "export" {
        for arg in &argv[1..] {
            if let Some(eq) = arg.find('=') {
                let name = &arg[..eq];
                let val = &arg[eq + 1..];
                vars.set(name, val);
                vars.export(name);
            } else {
                vars.export(arg);
            }
        }
        return 0;
    }

    // Built-in: unset
    if cmd == "unset" {
        for arg in &argv[1..] {
            vars.unset(arg);
        }
        return 0;
    }

    // Built-in: set (print all variables)
    if cmd == "set" && argv.len() == 1 {
        for (k, v) in &vars.entries {
            println!("{}={}", k, v);
        }
        return 0;
    }

    // Built-in: read
    if cmd == "read" {
        let var_name = argv.get(1).map(|s| s.as_str()).unwrap_or("REPLY");
        let mut line = [0u8; 256];
        let n = syscall::read(0, &mut line);
        if n <= 0 { return 1; }
        let mut end = n as usize;
        if end > 0 && line[end - 1] == b'\n' { end -= 1; }
        let val = unsafe { core::str::from_utf8_unchecked(&line[..end]) };
        vars.set(var_name, val);
        return 0;
    }

    // Built-in: true
    if cmd == "true" { return 0; }

    // Built-in: false
    if cmd == "false" { return 1; }

    // Built-in: clear
    if cmd == "clear" {
        syscall::write(1, b"\x1b[2J\x1b[H");
        return 0;
    }

    // Built-in: test / [
    if cmd == "test" || cmd == "[" {
        return builtin_test(argv);
    }

    // Built-in: . (source)
    if cmd == "." || cmd == "source" {
        if let Some(path) = argv.get(1) {
            return run_script(path, &argv[1..], vars);
        }
        println!(".: filename argument required");
        return 2;
    }

    // External command: fork + exec
    let pid = syscall::fork();
    if pid < 0 {
        println!("sh: fork failed");
        return 1;
    }
    if pid == 0 {
        // Child process

        // Apply assignments to environment (for this command only)
        for (k, v) in assignments {
            vars.set(k, v);
        }

        // Set up redirections
        if let Some(f) = stdin_file {
            syscall::close(0);
            let mut buf = [0u8; 128];
            let len = f.len().min(127);
            buf[..len].copy_from_slice(&f.as_bytes()[..len]);
            if syscall::open(buf.as_ptr(), syscall::O_RDONLY) < 0 {
                println!("sh: cannot open {}", f);
                syscall::exit(1);
            }
        }
        if let Some(f) = stdout_file {
            syscall::close(1);
            let mut buf = [0u8; 128];
            let len = f.len().min(127);
            buf[..len].copy_from_slice(&f.as_bytes()[..len]);
            let flags = if append {
                syscall::O_WRONLY | syscall::O_CREATE
            } else {
                syscall::O_WRONLY | syscall::O_CREATE
            };
            if syscall::open(buf.as_ptr(), flags) < 0 {
                println!("sh: cannot open {}", f);
                syscall::exit(1);
            }
        }

        // Build argv for exec
        let mut argv_bufs: Vec<[u8; 64]> = Vec::new();
        for arg in argv {
            let mut buf = [0u8; 64];
            let len = arg.len().min(63);
            buf[..len].copy_from_slice(&arg.as_bytes()[..len]);
            argv_bufs.push(buf);
        }
        let mut argv_ptrs: Vec<*const u8> = Vec::new();
        for buf in &argv_bufs {
            argv_ptrs.push(buf.as_ptr());
        }
        argv_ptrs.push(core::ptr::null());

        syscall::exec(argv_bufs[0].as_ptr(), argv_ptrs.as_ptr());
        println!("sh: exec {} failed", cmd);
        syscall::exit(127);
    }

    // Parent: wait
    let mut status: i32 = 0;
    syscall::wait(&mut status);
    status
}

fn exec_pipeline(cmds: &[Cmd], vars: &mut Vars) -> i32 {
    if cmds.len() == 1 {
        return exec_cmd(&cmds[0], vars);
    }

    let mut prev_read_fd: i32 = -1;
    let mut pids: Vec<i32> = Vec::new();

    for (i, cmd) in cmds.iter().enumerate() {
        let is_last = i == cmds.len() - 1;
        let mut pipefd = [0i32; 2];
        if !is_last {
            syscall::pipe(&mut pipefd);
        }

        let pid = syscall::fork();
        if pid == 0 {
            // Child
            if prev_read_fd >= 0 {
                syscall::close(0);
                syscall::dup(prev_read_fd);
                syscall::close(prev_read_fd);
            }
            if !is_last {
                syscall::close(1);
                syscall::dup(pipefd[1]);
                syscall::close(pipefd[0]);
                syscall::close(pipefd[1]);
            }
            let status = exec_cmd(cmd, vars);
            syscall::exit(status);
        }

        pids.push(pid);
        if prev_read_fd >= 0 { syscall::close(prev_read_fd); }
        if !is_last {
            syscall::close(pipefd[1]);
            prev_read_fd = pipefd[0];
        }
    }

    // Wait for all children
    let mut last_status: i32 = 0;
    for _ in &pids {
        let mut s: i32 = 0;
        syscall::wait(&mut s);
        last_status = s;
    }
    last_status
}

// ---- test builtin ----------------------------------------------------------

fn builtin_test(argv: &[String]) -> i32 {
    let is_bracket = argv[0] == "[";
    let args: &[String] = if is_bracket {
        if argv.last().map(|s| s.as_str()) == Some("]") {
            &argv[1..argv.len() - 1]
        } else {
            &argv[1..]
        }
    } else {
        &argv[1..]
    };

    let result = eval_test(args);
    if result { 0 } else { 1 }
}

fn eval_test(args: &[String]) -> bool {
    match args.len() {
        0 => false,
        1 => !args[0].is_empty(),
        2 => {
            let op = args[0].as_str();
            match op {
                "-n" => !args[1].is_empty(),
                "-z" => args[1].is_empty(),
                "!" => !eval_test(&args[1..]),
                "-f" | "-e" => {
                    let mut buf = [0u8; 128];
                    let len = args[1].len().min(127);
                    buf[..len].copy_from_slice(&args[1].as_bytes()[..len]);
                    let fd = syscall::open(buf.as_ptr(), syscall::O_RDONLY);
                    if fd >= 0 { syscall::close(fd); true } else { false }
                }
                "-d" => {
                    let mut buf = [0u8; 128];
                    let len = args[1].len().min(127);
                    buf[..len].copy_from_slice(&args[1].as_bytes()[..len]);
                    let fd = syscall::open(buf.as_ptr(), syscall::O_RDONLY);
                    if fd >= 0 {
                        let mut st = rxv6_user::syscall::Stat {
                            file_type: rxv6_user::syscall::FileType::None,
                            dev: 0, ino: 0, nlink: 0, size: 0,
                        };
                        syscall::fstat(fd, &mut st);
                        syscall::close(fd);
                        st.file_type == rxv6_user::syscall::FileType::Dir
                    } else { false }
                }
                _ => !args[0].is_empty(), // fallback: non-empty string is true
            }
        }
        3.. => {
            let op = args[1].as_str();
            match op {
                "=" | "==" => args[0] == args[2],
                "!=" => args[0] != args[2],
                "-eq" => parse_int(&args[0]) == parse_int(&args[2]),
                "-ne" => parse_int(&args[0]) != parse_int(&args[2]),
                "-lt" => parse_int(&args[0]) < parse_int(&args[2]),
                "-le" => parse_int(&args[0]) <= parse_int(&args[2]),
                "-gt" => parse_int(&args[0]) > parse_int(&args[2]),
                "-ge" => parse_int(&args[0]) >= parse_int(&args[2]),
                _ => false,
            }
        }
    }
}

fn parse_int(s: &str) -> i32 {
    let bytes = s.as_bytes();
    let mut n: i32 = 0;
    let mut i = 0;
    let neg = !bytes.is_empty() && bytes[0] == b'-';
    if neg { i = 1; }
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        n = n.wrapping_mul(10).wrapping_add((bytes[i] - b'0') as i32);
        i += 1;
    }
    if neg { -n } else { n }
}

// ---- Script execution ------------------------------------------------------

fn run_script(path: &str, args: &[String], vars: &mut Vars) -> i32 {
    let mut path_buf = [0u8; 128];
    let len = path.len().min(127);
    path_buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    let fd = syscall::open(path_buf.as_ptr(), syscall::O_RDONLY);
    if fd < 0 {
        println!("sh: cannot open {}", path);
        return 1;
    }

    // Read entire file
    let mut content = String::new();
    let mut buf = [0u8; 256];
    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 { break; }
        for &b in &buf[..n as usize] {
            content.push(b as char);
        }
    }
    syscall::close(fd);

    // Save and set script args
    let old_args = core::mem::replace(&mut vars.script_args, args.to_vec());

    let status = run_string(&content, vars);

    vars.script_args = old_args;
    status
}

fn run_string(input: &str, vars: &mut Vars) -> i32 {
    let tokens = tokenize(input, vars);
    let mut parser = Parser::new(tokens);
    let cmd = parser.parse_list();
    let status = exec_cmd(&cmd, vars);
    vars.last_status = status;
    status
}

// ---- History (same as before, for interactive mode) ------------------------

struct History {
    lines: [[u8; BUFSIZE]; HISTORY_SIZE],
    lens: [usize; HISTORY_SIZE],
    count: usize,
    browse: usize,
}

impl History {
    const fn new() -> Self {
        History {
            lines: [[0; BUFSIZE]; HISTORY_SIZE],
            lens: [0; HISTORY_SIZE],
            count: 0,
            browse: 0,
        }
    }

    fn push(&mut self, line: &[u8], len: usize) {
        if len == 0 { return; }
        let idx = self.count % HISTORY_SIZE;
        self.lines[idx][..len].copy_from_slice(&line[..len]);
        self.lines[idx][len] = 0;
        self.lens[idx] = len;
        self.count += 1;
        self.browse = self.count;
    }

    fn get(&self, idx: usize) -> Option<(&[u8], usize)> {
        if idx >= self.count { return None; }
        let oldest = if self.count > HISTORY_SIZE { self.count - HISTORY_SIZE } else { 0 };
        if idx < oldest { return None; }
        let slot = idx % HISTORY_SIZE;
        Some((&self.lines[slot], self.lens[slot]))
    }
}

static mut HIST: History = History::new();

fn getline_with_history(buf: &mut [u8]) -> i32 {
    syscall::set_raw_mode(true);
    unsafe { HIST.browse = HIST.count; }
    let mut pos = 0usize;
    let mut len = 0usize;

    loop {
        let mut c = [0u8; 1];
        let n = syscall::read(0, &mut c);
        if n <= 0 { syscall::set_raw_mode(false); return -1; }

        match c[0] {
            b'\n' | b'\r' => {
                syscall::write(1, b"\n");
                buf[len] = b'\n';
                len += 1;
                buf[len] = 0;
                syscall::set_raw_mode(false);
                return len as i32;
            }
            0x7F | 0x08 => {
                if pos > 0 {
                    for i in pos..len { buf[i - 1] = buf[i]; }
                    pos -= 1; len -= 1;
                    syscall::write(1, b"\x08");
                    syscall::write(1, &buf[pos..len]);
                    syscall::write(1, b" ");
                    for _ in 0..len - pos + 1 { syscall::write(1, b"\x08"); }
                }
            }
            0x15 => { while pos > 0 { syscall::write(1, b"\x08 \x08"); pos -= 1; } len = 0; }
            0x01 => { while pos > 0 { syscall::write(1, b"\x08"); pos -= 1; } }
            0x05 => { if pos < len { syscall::write(1, &buf[pos..len]); pos = len; } }
            0x04 => { if len == 0 { syscall::set_raw_mode(false); return 0; } }
            0x1B => {
                let mut seq = [0u8; 2];
                if syscall::read(0, &mut seq[..1]) <= 0 { continue; }
                if seq[0] == b'[' {
                    if syscall::read(0, &mut seq[1..2]) <= 0 { continue; }
                    match seq[1] {
                        b'A' => unsafe {
                            if HIST.browse > 0 {
                                let oldest = if HIST.count > HISTORY_SIZE { HIST.count - HISTORY_SIZE } else { 0 };
                                if HIST.browse > oldest {
                                    HIST.browse -= 1;
                                    replace_line(buf, &mut pos, &mut len);
                                }
                            }
                        }
                        b'B' => unsafe {
                            if HIST.browse < HIST.count {
                                HIST.browse += 1;
                                replace_line(buf, &mut pos, &mut len);
                            }
                        }
                        b'C' => { if pos < len { syscall::write(1, b"\x1b[C"); pos += 1; } }
                        b'D' => { if pos > 0 { syscall::write(1, b"\x1b[D"); pos -= 1; } }
                        b'H' => { while pos > 0 { syscall::write(1, b"\x08"); pos -= 1; } }
                        b'F' => { if pos < len { syscall::write(1, &buf[pos..len]); pos = len; } }
                        b'3' => {
                            let mut tilde = [0u8; 1];
                            let _ = syscall::read(0, &mut tilde);
                            if tilde[0] == b'~' && pos < len {
                                for i in pos..len - 1 { buf[i] = buf[i + 1]; }
                                len -= 1;
                                syscall::write(1, &buf[pos..len]);
                                syscall::write(1, b" ");
                                for _ in 0..len - pos + 1 { syscall::write(1, b"\x08"); }
                            }
                        }
                        c if c >= b'0' && c <= b'9' => {
                            let mut t = [0u8; 1];
                            let _ = syscall::read(0, &mut t);
                        }
                        _ => {}
                    }
                }
            }
            c if c >= 0x20 => {
                if len < buf.len() - 2 {
                    for i in (pos..len).rev() { buf[i + 1] = buf[i]; }
                    buf[pos] = c; len += 1; pos += 1;
                    syscall::write(1, &buf[pos - 1..len]);
                    for _ in 0..len - pos { syscall::write(1, b"\x08"); }
                }
            }
            _ => {}
        }
    }
}

fn replace_line(buf: &mut [u8], pos: &mut usize, len: &mut usize) {
    while *pos > 0 { syscall::write(1, b"\x08"); *pos -= 1; }
    for _ in 0..*len { syscall::write(1, b" "); }
    for _ in 0..*len { syscall::write(1, b"\x08"); }

    unsafe {
        if HIST.browse >= HIST.count {
            *len = 0; *pos = 0;
        } else if let Some((line, line_len)) = HIST.get(HIST.browse) {
            buf[..line_len].copy_from_slice(&line[..line_len]);
            buf[line_len] = 0;
            *len = line_len; *pos = line_len;
            syscall::write(1, &buf[..*len]);
        }
    }
}

// ---- Main ------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn rust_main(argc: i32, argv: *const *const u8) -> i32 {
    let mut vars = Vars::new();

    // Set up script args
    if argc > 0 {
        for i in 0..argc {
            let arg = unsafe { rxv6_user::cstr_to_str(*argv.add(i as usize)) };
            vars.script_args.push(String::from(arg));
        }
    } else {
        vars.script_args.push(String::from("sh"));
    }

    // Ignore SIGINT in the shell itself (children get default)
    syscall::signal(syscall::SIGINT, syscall::SIG_IGN);
    syscall::signal(syscall::SIGQUIT, syscall::SIG_IGN);

    // Check for -c flag: sh -c "command"
    if argc >= 3 {
        let arg1 = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
        if arg1 == "-c" {
            let cmd_str = unsafe { rxv6_user::cstr_to_str(*argv.add(2)) };
            let status = run_string(cmd_str, &mut vars);
            return status;
        }
    }

    // Check for script file argument
    if argc >= 2 {
        let arg1 = unsafe { rxv6_user::cstr_to_str(*argv.add(1)) };
        if arg1 != "-c" {
            let args: Vec<String> = (1..argc).map(|i| {
                String::from(unsafe { rxv6_user::cstr_to_str(*argv.add(i as usize)) })
            }).collect();
            return run_script(arg1, &args, &mut vars);
        }
    }

    // Interactive mode
    let mut buf = [0u8; BUFSIZE];
    loop {
        // Prompt
        let prompt = vars.get("PS1").unwrap_or("$ ");
        print!("{}", prompt);

        let n = getline_with_history(&mut buf);
        if n <= 0 {
            println!("");
            break;
        }
        let line_len = n as usize;
        // Strip trailing newline
        let end = if line_len > 0 && buf[line_len - 1] == b'\n' { line_len - 1 } else { line_len };
        if end == 0 { continue; }

        let line = unsafe { core::str::from_utf8_unchecked(&buf[..end]) };
        if line.trim().is_empty() { continue; }

        // Save to history
        unsafe { HIST.push(&buf[..end], end); }

        let status = run_string(line, &mut vars);
        vars.last_status = status;
    }
    0
}
