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
    End,
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
        nextc == ch
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
    pub fn read_newline(&mut self) -> Token {
        self.peek_next();
        Token::new(TokenKind::Newline, "", self.cur_line)
    }
    pub fn read_symbol(&mut self) -> Token {
        let c = self.peek_next();
        let mut sym = String::new();
        sym.push(c);
        Token::new(TokenKind::Symbol, sym.as_str(), self.cur_line)
    }

    pub fn read_token(&mut self) -> Option<Token> {
        match self.peek_get() {
            Some(&c) => {
                match c {
                    'a'...'z' | 'A'...'Z' | '_' => Some(self.read_identifier()),
                    ' ' | '\t' => {
                        self.peek_next();
                        self.read_token()
                    }
                    '\n' => Some(self.read_newline()),
                    _ => Some(self.read_symbol()),
                }
            }
            None => None as Option<Token>,
        }
    }
}
