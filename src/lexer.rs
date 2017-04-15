use std::io::{BufRead, BufReader};
use std::io;
use std::io::prelude::*;
use std::iter;
use std::str;
use std::collections::VecDeque;

pub enum TokenKind {
    Identifier,
    IntNumber,
    FloatNumber,
    String,
    Char,
    Symbol,
    Newline,
}

pub struct Token {
    pub kind: TokenKind,
    pub val: String,
    pub line: i32,
}

impl Token {
    pub fn new(kind: TokenKind, val: &str, line: i32) -> Token {
        Token {
            kind: kind,
            val: val.to_string(),
            line: line,
        }
    }
}

pub struct Lexer<'a> {
    cur_line: i32,
    filename: String,
    peek: iter::Peekable<str::Chars<'a>>,
    buf: VecDeque<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(filename: String, input: &'a str) -> Lexer<'a> {
        Lexer {
            cur_line: 0,
            filename: filename.to_string(),
            peek: input.chars().peekable(),
            buf: VecDeque::new(),
        }
    }
    pub fn get_filename(self) -> String {
        self.filename
    }

    fn peek_get(&mut self) -> Option<&char> {
        if self.buf.len() > 0 {
            self.buf.front()
        } else {
            self.peek.peek()
        }
    }
    fn peek_next(&mut self) -> char {
        if self.buf.len() > 0 {
            self.buf.pop_front().unwrap()
        } else {
            self.peek.next().unwrap()
        }
    }
    fn peek_unget(&mut self, ch: char) {
        self.buf.push_back(ch);
    }
    fn peek_next_char_is(&mut self, ch: char) -> bool {
        let nextc = self.peek_next();
        self.peek_unget(nextc);
        nextc == ch
    }
    fn peek_char_is(&mut self, ch: char) -> bool {
        let peekc = self.peek_get().unwrap();
        *peekc == ch
    }

    pub fn read_identifier(&mut self) -> Token {
        let mut ident = String::new();
        loop {
            match self.peek_get() {
                Some(&c) => {
                    match c {
                        'a'...'z' | 'A'...'Z' | '_' | '0'...'9' => ident.push(c),
                        _ => break,
                    }
                }
                _ => break,
            };
            self.peek_next();
        }
        Token::new(TokenKind::Identifier, ident.as_str(), self.cur_line)
    }
    fn read_number_literal(&mut self) -> Token {
        let mut num = String::new();
        let mut is_float = false;
        loop {
            match self.peek_get() {
                Some(&c) => {
                    match c {
                        '.' | '0'...'9' => {
                            num.push(c);
                            if c == '.' {
                                is_float = true;
                            }
                        }
                        _ => break,
                    }
                }
                _ => break,
            };
            self.peek_next();
        }
        if is_float {
            Token::new(TokenKind::FloatNumber, num.as_str(), self.cur_line)
        } else {
            Token::new(TokenKind::IntNumber, num.as_str(), self.cur_line)
        }
    }
    pub fn read_newline(&mut self) -> Token {
        self.peek_next();
        Token::new(TokenKind::Newline, "", self.cur_line)
    }
    pub fn read_symbol(&mut self) -> Token {
        let c = self.peek_next();
        let mut sym = String::new();
        sym.push(c);
        match c {
            '+' | '-' | '*' | '/' | '%' | '=' | '^' | '!' => {
                if self.peek_char_is('=') {
                    sym.push(self.peek_next());
                }
            }
            '<' | '>' | '&' | '|' => {
                if self.peek_char_is(c) {
                    sym.push(self.peek_next());
                }
                if self.peek_char_is('=') {
                    sym.push(self.peek_next());
                }
            }
            '.' => {
                if self.peek_char_is('.') && self.peek_next_char_is('.') {
                    sym.push(self.peek_next());
                    sym.push(self.peek_next());
                }
            }
            _ => {}
        };
        Token::new(TokenKind::Symbol, sym.as_str(), self.cur_line)
    }
    fn read_string_literal(&mut self) -> Token {
        self.peek_next();
        let mut s = String::new();
        while !self.peek_char_is('\"') {
            s.push(self.peek_next());
        }
        self.peek_next();
        Token::new(TokenKind::String, s.as_str(), self.cur_line)
    }
    fn read_char_literal(&mut self) -> Token {
        self.peek_next();
        let mut s = String::new();
        while !self.peek_char_is('\'') {
            s.push(self.peek_next());
        }
        self.peek_next();
        Token::new(TokenKind::Char, s.as_str(), self.cur_line)
    }

    pub fn do_read_token(&mut self) -> Option<Token> {
        match self.peek_get() {
            Some(&c) => {
                match c {
                    'a'...'z' | 'A'...'Z' | '_' => Some(self.read_identifier()),
                    ' ' | '\t' => {
                        self.peek_next();
                        self.read_token()
                    }
                    '0'...'9' => Some(self.read_number_literal()),
                    '\"' => Some(self.read_string_literal()),
                    '\'' => Some(self.read_char_literal()),
                    '\n' => Some(self.read_newline()),
                    _ => Some(self.read_symbol()),
                }
            }
            None => None as Option<Token>,
        }
    }
    pub fn read_token(&mut self) -> Option<Token> {
        let t = self.do_read_token();
        match t {
            Some(tok) => {
                match tok.kind {
                    TokenKind::Newline => self.read_token(),
                    _ => Some(tok),
                }
            }
            _ => t,
        }
    }
}
