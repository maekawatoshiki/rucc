use std::io::{BufRead, BufReader};
use std::io;
use std::io::prelude::*;
use std::iter;
use std::str;

pub enum TokenKind {
    Identifier,
    IntNumber,
    FloatNumber,
    String,
    Char,
    Symbol,
    End,
}

pub struct Token {
    kind: TokenKind,
    val: String,
    line: i32,
}

pub struct Lexer<'a> {
    cur_line: i32,
    filename: String,
    peek: iter::Peekable<str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(filename: String, input: &'a str) -> Lexer<'a> {
        let l = Lexer {
            cur_line: 0,
            filename: filename.to_string(),
            peek: input.chars().peekable(),
        };
        l
    }
    pub fn get_filename(self) -> String {
        self.filename
    }
    pub fn read_token(&mut self) -> Token {
        // TODO: tentatively
        println!("{}", self.peek.peek().unwrap());
        self.peek.next();

        Token {
            kind: TokenKind::End,
            val: "".to_string(),
            line: 0,
        }
    }
}
