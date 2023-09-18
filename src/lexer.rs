use error;
use node::Bits;
use parser;
use parser::{Error, ParseR};
use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path;
use std::process;
use std::str;

extern crate ansi_term;
use self::ansi_term::{Colour, Style};

#[derive(Debug, Clone)]
pub enum Macro {
    // Vec<Token> -> macro body
    Object(Vec<Token>),
    FuncLike(Vec<Token>),
}

#[derive(PartialEq, Debug, Clone)]
pub enum Keyword {
    Typedef,
    Extern,
    Static,
    Auto,
    Restrict,
    Register,
    Const,
    ConstExpr,
    Volatile,
    Void,
    Signed,
    Unsigned,
    Char,
    Int,
    Short,
    Long,
    Float,
    Double,
    Struct,
    Enum,
    Union,
    Noreturn,
    Inline,
    If,
    Else,
    For,
    Do,
    While,
    Switch,
    Case,
    Default,
    Goto,
    Break,
    Continue,
    Return,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Symbol {
    OpeningParen,
    ClosingParen,
    OpeningBrace,
    ClosingBrace,
    OpeningBoxBracket,
    ClosingBoxBracket,
    Comma,
    Semicolon,
    Colon,
    Point,
    Arrow,
    Inc,
    Dec,
    Add,
    Sub,
    Asterisk,
    Div,
    Mod,
    Not,
    BitwiseNot,
    Ampersand,
    Shl,
    Shr,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    Xor,
    Or,
    LAnd,
    LOr,
    Question,
    Assign,
    AssignAdd,
    AssignSub,
    AssignMul,
    AssignDiv,
    AssignMod,
    AssignShl,
    AssignShr,
    AssignAnd,
    AssignXor,
    AssignOr,
    Hash,
    Vararg,
    Sizeof,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Pos {
    pub line: usize,
    pub pos: usize,
}

impl Pos {
    pub fn new(line: usize, pos: usize) -> Pos {
        Pos {
            line: line,
            pos: pos,
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum TokenKind {
    MacroParam,
    Keyword(Keyword),
    Identifier(String),
    IntNumber(i64, Bits),
    FloatNumber(f64),
    String(String),
    Char(char),
    Symbol(Symbol),
    Newline,
}

macro_rules! ident_val {
    ($e:expr) => {
        match &$e.kind {
            &TokenKind::Identifier(ref ident) => ident.to_string(),
            _ => "".to_string(),
        }
    };
}
macro_rules! ident_mut_val {
    ($e:expr) => {
        match &mut $e.kind {
            &mut TokenKind::Identifier(ref mut ident) => ident,
            _ => panic!(),
        }
    };
}
macro_rules! retrieve_str {
    ($e:expr) => {
        match &$e.kind {
            &TokenKind::String(ref s) => s.to_string(),
            _ => panic!(),
        }
    };
}
macro_rules! matches {
    ($e:expr, $p:pat) => {
        match $e {
            $p => true,
            _ => false,
        }
    };
}

#[derive(PartialEq, Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub space: bool, // leading space
    pub macro_position: usize,
    pub hideset: HashSet<String>,
    pub pos: Pos,
}

impl Token {
    pub fn new(kind: TokenKind, macro_position: usize, pos: usize, line: usize) -> Token {
        Token {
            kind: kind,
            space: false,
            macro_position: macro_position,
            hideset: HashSet::new(),
            pos: Pos::new(line, pos),
        }
    }
    pub fn add_hideset(&mut self, s: String) {
        self.hideset.insert(s);
    }
}

#[derive(Clone)]
pub struct Lexer {
    pub cur_line: VecDeque<usize>,
    filename: VecDeque<String>,
    macro_map: HashMap<String, Macro>,
    pub peek: VecDeque<Vec<u8>>,
    pub peek_pos: VecDeque<usize>,
    buf: VecDeque<VecDeque<Token>>,
    cond_stack: Vec<bool>,
}

impl Lexer {
    pub fn new(filename: String) -> Lexer {
        let mut buf = VecDeque::new();
        buf.push_back(VecDeque::new());

        let mut file = if let Ok(ok) = OpenOptions::new().read(true).open(filename.to_string()) {
            ok
        } else {
            println!(
                "{} not found such file '{}'",
                Colour::Red.bold().paint("error:"),
                Style::new().underline().paint(filename)
            );
            ::std::process::exit(0)
        };

        let mut file_body = String::new();
        match file.read_to_string(&mut file_body) {
            Ok(_) => (),
            Err(e) => {
                println!(
                    "an error occurred while reading file '{}'\n{} {}",
                    Style::new().underline().paint(filename),
                    Colour::Red.bold().paint("error:"),
                    e
                );
                ::std::process::exit(0)
            }
        };

        let mut rucc_header = OpenOptions::new()
            .read(true)
            .open("./include/rucc.h")
            .unwrap();
        let mut rucc_header_body = String::new();
        rucc_header
            .read_to_string(&mut rucc_header_body)
            .ok()
            .expect("cannot read file");
        let mut peek = VecDeque::new();
        unsafe {
            peek.push_back(file_body.as_mut_vec().clone());
            peek.push_back(rucc_header_body.as_mut_vec().clone());
        }

        let mut peek_pos = VecDeque::new();
        peek_pos.push_back(0);
        peek_pos.push_back(0);

        let mut filenames = VecDeque::new();
        filenames.push_back(filename);
        filenames.push_back("rucc.h".to_string());

        let mut cur_line = VecDeque::new();
        cur_line.push_back(1);
        cur_line.push_back(1);

        Lexer {
            cur_line: cur_line,
            filename: filenames,
            macro_map: HashMap::new(),
            peek: peek,
            peek_pos: peek_pos,
            buf: buf,
            cond_stack: Vec::new(),
        }
    }
    pub fn get_filename(&self) -> String {
        self.filename.back().unwrap().to_owned()
    }
    pub fn get_cur_pos(&self) -> Pos {
        Pos::new(*self.get_cur_line(), *self.peek_pos.back().unwrap())
    }
    pub fn get_cur_line(&self) -> &usize {
        self.cur_line.back().unwrap()
    }
    fn peek_get(&mut self) -> ParseR<char> {
        let peek = self.peek.back_mut().unwrap();
        let peek_pos = *self.peek_pos.back_mut().unwrap();
        if peek_pos >= peek.len() {
            Err(Error::EOF)
        } else {
            Ok(peek[peek_pos] as char)
        }
    }
    fn peek_next(&mut self) -> ParseR<char> {
        let peek = self.peek.back().unwrap();
        let peek_pos = self.peek_pos.back_mut().unwrap();
        if *peek_pos >= peek.len() {
            return Err(Error::EOF);
        }
        let c = peek[*peek_pos] as char;
        *peek_pos += 1;
        if c == '\n' {
            *self.cur_line.back_mut().unwrap() += 1;
        }
        Ok(c)
    }
    fn peek_next_char_is(&mut self, ch: char) -> ParseR<bool> {
        let peek = self.peek.back_mut().unwrap();
        let peek_pos = self.peek_pos.back_mut().unwrap();
        if *peek_pos >= peek.len() {
            Err(Error::EOF)
        } else {
            let nextc = peek[*peek_pos + 1] as char;
            Ok(nextc == ch)
        }
    }
    fn peek_char_is(&mut self, ch: char) -> ParseR<bool> {
        let peekc = self.peek_get()?;
        Ok(peekc == ch)
    }

    pub fn peek_keyword_token_is(&mut self, expect: Keyword) -> ParseR<bool> {
        let peek = self.peek()?;
        Ok(peek.kind == TokenKind::Keyword(expect))
    }
    pub fn peek_symbol_token_is(&mut self, expect: Symbol) -> ParseR<bool> {
        let peek = self.peek()?;
        Ok(peek.kind == TokenKind::Symbol(expect))
    }
    pub fn next_symbol_token_is(&mut self, expect: Symbol) -> ParseR<bool> {
        let peek = self.get()?;
        let next = self.get()?;
        let next_token_is_expected = next.kind == TokenKind::Symbol(expect);
        self.unget(next);
        self.unget(peek);
        Ok(next_token_is_expected)
    }
    pub fn skip_keyword(&mut self, keyword: Keyword) -> ParseR<bool> {
        let tok = self.get()?;
        if tok.kind == TokenKind::Keyword(keyword) {
            return Ok(true);
        }
        self.unget(tok);
        Ok(false)
    }
    pub fn skip_symbol(&mut self, sym: Symbol) -> ParseR<bool> {
        let tok = self.get()?;
        if tok.kind == TokenKind::Symbol(sym) {
            return Ok(true);
        }
        self.unget(tok);
        Ok(false)
    }
    pub fn expect_skip_keyword(&mut self, expect: Keyword) -> ParseR<bool> {
        self.skip_keyword(expect)
    }
    pub fn expect_skip_symbol(&mut self, expect: Symbol) -> ParseR<bool> {
        self.skip_symbol(expect)
    }
    pub fn unget(&mut self, t: Token) {
        self.buf.back_mut().unwrap().push_back(t);
    }
    pub fn unget_all(&mut self, tv: &Vec<Token>) {
        let buf = self.buf.back_mut().unwrap();
        buf.extend(tv.iter().rev().map(|tok| tok.clone()));
    }

    pub fn read_identifier(&mut self, c: char) -> ParseR<Token> {
        let mut ident = "".to_string();
        ident.push(c);
        let pos = *self.peek_pos.back().unwrap();
        loop {
            let c = self.peek_next()?;
            if c.is_alphanumeric() || c == '_' {
                ident.push(c);
            } else {
                *self.peek_pos.back_mut().unwrap() -= 1;
                break;
            }
        }
        Ok(Token::new(
            TokenKind::Identifier(ident),
            0,
            pos,
            *self.get_cur_line(),
        ))
    }
    fn read_number_literal(&mut self, c: char) -> ParseR<Token> {
        let mut num = "".to_string();
        num.push(c);
        let mut is_float = false;
        let mut last = self.peek_get()?;
        let pos = *self.peek_pos.back().unwrap();
        loop {
            let c = self.peek_next()?;
            num.push(c);
            is_float = is_float || c == '.';
            let is_f = "eEpP".contains(last) && "+-".contains(c);
            if !c.is_alphanumeric() && c != '.' && !is_f {
                is_float = is_float || is_f;
                num.pop();
                *self.peek_pos.back_mut().unwrap() -= 1;
                break;
            }
            last = c;
        }

        if is_float {
            // TODO: now rucc ignores suffix
            num = num
                .trim_end_matches(|c| match c {
                    'a'..='z' | 'A'..='Z' | '+' | '-' => true,
                    _ => false,
                })
                .to_string();
            let f: f64 = num.parse().unwrap();
            Ok(Token::new(
                TokenKind::FloatNumber(f),
                0,
                pos,
                *self.get_cur_line(),
            ))
        } else {
            // TODO: suffix supporting
            let i = if num.len() > 2 && num.chars().nth(1).unwrap() == 'x' {
                self.read_hex_num(&num[2..]).0
            } else if num.chars().nth(0).unwrap() == '0' {
                self.read_oct_num(&num[1..]).0
            } else {
                self.read_dec_num(num.as_str()).0
            };

            let max_32bits = 0xffffffff;
            let bits = if 0 == (i & !max_32bits) {
                Bits::Bits32
            } else {
                Bits::Bits64
            };
            Ok(Token::new(
                TokenKind::IntNumber(i, bits),
                0,
                pos,
                *self.get_cur_line(),
            ))
        }
    }
    fn read_dec_num(&mut self, num_literal: &str) -> (i64, String) {
        let mut suffix = "".to_string();
        let n = num_literal.chars().fold(0, |n, c| match c {
            '0'..='9' => n * 10 + c.to_digit(10).unwrap() as u64,
            _ => {
                suffix.push(c);
                n
            }
        });
        (n as i64, suffix)
    }
    fn read_oct_num(&mut self, num_literal: &str) -> (i64, String) {
        let mut suffix = "".to_string();
        let n = num_literal.chars().fold(0, |n, c| match c {
            '0'..='7' => n * 8 + c.to_digit(8).unwrap() as u64,
            _ => {
                suffix.push(c);
                n
            }
        });
        (n as i64, suffix)
    }
    fn read_hex_num(&mut self, num_literal: &str) -> (i64, String) {
        let mut suffix = "".to_string();
        let n = num_literal.chars().fold(0, |n, c| match c {
            '0'..='9' | 'A'..='F' | 'a'..='f' => n * 16 + c.to_digit(16).unwrap() as u64,
            _ => {
                suffix.push(c);
                n
            }
        });
        (n as i64, suffix)
    }
    pub fn read_newline(&mut self) -> ParseR<Token> {
        Ok(Token::new(
            TokenKind::Newline,
            0,
            *self.peek_pos.back().unwrap(),
            *self.get_cur_line(),
        ))
    }
    pub fn read_symbol(&mut self, c: char) -> ParseR<Token> {
        let pos = *self.peek_pos.back().unwrap();
        let mut sym = "".to_string();
        sym.push(c);
        match c {
            '+' | '-' => {
                if self.peek_char_is('=')?
                    || self.peek_char_is('>')?
                    || self.peek_char_is('+')?
                    || self.peek_char_is('-')?
                {
                    sym.push(self.peek_next()?);
                }
            }
            '*' | '/' | '%' | '=' | '^' | '!' => {
                if self.peek_char_is('=')? {
                    sym.push(self.peek_next()?);
                }
            }
            '<' | '>' | '&' | '|' => {
                if self.peek_char_is(c)? {
                    sym.push(self.peek_next()?);
                }
                if self.peek_char_is('=')? {
                    sym.push(self.peek_next()?);
                }
            }
            '.' => {
                if self.peek_char_is('.')? && self.peek_next_char_is('.')? {
                    sym.push(self.peek_next()?);
                    sym.push(self.peek_next()?);
                }
            }
            _ => {}
        };
        Ok(Token::new(
            TokenKind::Identifier(sym),
            0,
            pos,
            *self.get_cur_line(),
        ))
    }
    fn read_escaped_char(&mut self) -> ParseR<char> {
        let c = self.peek_next()?;
        match c {
            '\'' | '"' | '?' | '\\' => Ok(c),
            'a' => Ok('\x07'),
            'b' => Ok('\x08'),
            'f' => Ok('\x0c'),
            'n' => Ok('\x0a'),
            'r' => Ok('\x0d'),
            't' => Ok('\x09'),
            'v' => Ok('\x0b'),
            'x' => {
                let mut hex = "".to_string();
                loop {
                    let c = self.peek_get()?;
                    if c.is_alphanumeric() {
                        hex.push(c);
                    } else {
                        break;
                    }
                    self.peek_next()?;
                }
                Ok(self.read_hex_num(hex.as_str()).0 as i32 as u8 as char)
            }
            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' => {
                // if '0', check whether octal number \nnn or null \0
                if self.peek_get()?.is_numeric() {
                    let mut oct = "".to_string();
                    oct.push(c);
                    loop {
                        let c = self.peek_next()?;
                        oct.push(c);
                        if !c.is_numeric() {
                            oct.pop();
                            break;
                        }
                    }
                    *self.peek_pos.back_mut().unwrap() -= 1;
                    Ok(self.read_oct_num(oct.as_str()).0 as i32 as u8 as char)
                } else {
                    assert!(c == '0');
                    Ok('\x00')
                }
            }
            _ => Ok(c),
        }
    }
    fn read_string_literal(&mut self) -> ParseR<Token> {
        let pos = *self.peek_pos.back().unwrap();
        let mut s = "".to_string();
        loop {
            match self.peek_next()? {
                '"' => break,
                '\\' => s.push(self.read_escaped_char()?),
                c => s.push(c),
            }
        }
        Ok(Token::new(
            TokenKind::String(s),
            0,
            pos,
            *self.get_cur_line(),
        ))
    }
    fn read_char_literal(&mut self) -> ParseR<Token> {
        let pos = *self.peek_pos.back().unwrap();
        let c = {
            let c = self.peek_next()?;
            if c == '\\' {
                self.read_escaped_char()?
            } else {
                c
            }
        };
        if self.peek_next()? != '\'' {
            error::error_exit(
                *self.cur_line.back().unwrap() as i32,
                "missing terminating \' char",
            );
        }
        Ok(Token::new(TokenKind::Char(c), 0, pos, *self.get_cur_line()))
    }

    pub fn do_read_token(&mut self) -> ParseR<Token> {
        if let Some(tok) = self.buf.back_mut().unwrap().pop_back() {
            return Ok(tok);
        }
        if self.buf.len() > 1 {
            return Err(Error::EOF);
        }

        match self.peek_next() {
            Ok(c) => {
                match c {
                    'a'..='z' | 'A'..='Z' | '_' => self.read_identifier(c),
                    ' ' | '\t' => {
                        self.do_read_token()
                            // set a leading space
                            .and_then(|tok| {
                                let mut t = tok;
                                t.space = true;
                                Ok(t)
                            })
                    }
                    '0'..='9' => self.read_number_literal(c),
                    '\"' => self.read_string_literal(),
                    '\'' => self.read_char_literal(),
                    '\n' => self.read_newline(),
                    '\\' => {
                        while self.peek_next()? != '\n' {}
                        self.do_read_token()
                    }
                    '/' => {
                        if self.peek_char_is('*')? {
                            self.peek_next()?; // *
                            let mut last = ' ';
                            while !(last == '*' && self.peek_char_is('/')?) {
                                last = self.peek_next()?;
                            }
                            self.peek_next()?; // /
                            self.do_read_token()
                        } else if self.peek_char_is('/')? {
                            self.peek_next()?; // /
                            while !self.peek_char_is('\n')? {
                                self.peek_next()?;
                            }
                            // self.peek_next()?; // \n
                            self.do_read_token()
                        } else {
                            self.read_symbol(c)
                        }
                    }
                    _ => self.read_symbol(c),
                }
            }
            _ => {
                if self.peek.len() > 1 {
                    self.peek.pop_back();
                    self.peek_pos.pop_back();
                    self.filename.pop_back();
                    self.cur_line.pop_back();
                    self.do_read_token()
                } else {
                    Err(Error::EOF)
                }
            }
        }
    }
    pub fn read_token(&mut self) -> ParseR<Token> {
        let token = self.do_read_token();
        token.and_then(|tok| match tok.kind {
            TokenKind::Newline => self.read_token(),
            TokenKind::Identifier(_) => Ok(self.convert_to_symbol(tok)),
            _ => Ok(tok),
        })
    }
    fn convert_to_symbol(&mut self, token: Token) -> Token {
        let pos = token.pos.pos;
        let line = token.pos.line;
        let val = ident_val!(token);

        if val == "sizeof" {
            return Token::new(TokenKind::Symbol(Symbol::Sizeof), 0, pos, line);
        }

        let symbol = match val.as_str() {
            "++" => TokenKind::Symbol(Symbol::Inc),
            "--" => TokenKind::Symbol(Symbol::Dec),
            "(" => TokenKind::Symbol(Symbol::OpeningParen),
            ")" => TokenKind::Symbol(Symbol::ClosingParen),
            "[" => TokenKind::Symbol(Symbol::OpeningBoxBracket),
            "]" => TokenKind::Symbol(Symbol::ClosingBoxBracket),
            "{" => TokenKind::Symbol(Symbol::OpeningBrace),
            "}" => TokenKind::Symbol(Symbol::ClosingBrace),
            "." => TokenKind::Symbol(Symbol::Point),
            "," => TokenKind::Symbol(Symbol::Comma),
            ";" => TokenKind::Symbol(Symbol::Semicolon),
            ":" => TokenKind::Symbol(Symbol::Colon),
            "->" => TokenKind::Symbol(Symbol::Arrow),
            "+" => TokenKind::Symbol(Symbol::Add),
            "-" => TokenKind::Symbol(Symbol::Sub),
            "!" => TokenKind::Symbol(Symbol::Not),
            "~" => TokenKind::Symbol(Symbol::BitwiseNot),
            "*" => TokenKind::Symbol(Symbol::Asterisk),
            "&" => TokenKind::Symbol(Symbol::Ampersand),
            "/" => TokenKind::Symbol(Symbol::Div),
            "%" => TokenKind::Symbol(Symbol::Mod),
            "<<" => TokenKind::Symbol(Symbol::Shl),
            ">>" => TokenKind::Symbol(Symbol::Shr),
            "<" => TokenKind::Symbol(Symbol::Lt),
            "<=" => TokenKind::Symbol(Symbol::Le),
            ">" => TokenKind::Symbol(Symbol::Gt),
            ">=" => TokenKind::Symbol(Symbol::Ge),
            "==" => TokenKind::Symbol(Symbol::Eq),
            "!=" => TokenKind::Symbol(Symbol::Ne),
            "^" => TokenKind::Symbol(Symbol::Xor),
            "|" => TokenKind::Symbol(Symbol::Or),
            "&&" => TokenKind::Symbol(Symbol::LAnd),
            "||" => TokenKind::Symbol(Symbol::LOr),
            "?" => TokenKind::Symbol(Symbol::Question),
            "=" => TokenKind::Symbol(Symbol::Assign),
            "+=" => TokenKind::Symbol(Symbol::AssignAdd),
            "-=" => TokenKind::Symbol(Symbol::AssignSub),
            "*=" => TokenKind::Symbol(Symbol::AssignMul),
            "/=" => TokenKind::Symbol(Symbol::AssignDiv),
            "%=" => TokenKind::Symbol(Symbol::AssignMod),
            "<<=" => TokenKind::Symbol(Symbol::AssignShl),
            ">>=" => TokenKind::Symbol(Symbol::AssignShr),
            "&=" => TokenKind::Symbol(Symbol::AssignAnd),
            "^=" => TokenKind::Symbol(Symbol::AssignXor),
            "|=" => TokenKind::Symbol(Symbol::AssignOr),
            "#" => TokenKind::Symbol(Symbol::Hash),
            "..." => TokenKind::Symbol(Symbol::Vararg),
            _ => return token,
        };

        Token::new(symbol, 0, pos, line)
    }
    fn maybe_convert_to_keyword(&mut self, token: Token) -> Token {
        let pos = token.pos.pos;
        let line = token.pos.line;
        let val = ident_val!(token);

        if val.len() > 0 && val.chars().nth(0).unwrap().is_alphanumeric() {
            let keyw = match val.as_str() {
                "typedef" => TokenKind::Keyword(Keyword::Typedef),
                "extern" => TokenKind::Keyword(Keyword::Extern),
                "auto" => TokenKind::Keyword(Keyword::Auto),
                "register" => TokenKind::Keyword(Keyword::Register),
                "static" => TokenKind::Keyword(Keyword::Static),
                "restrict" => TokenKind::Keyword(Keyword::Restrict),
                "const" => TokenKind::Keyword(Keyword::Const),
                "constexpr" => TokenKind::Keyword(Keyword::ConstExpr),
                "volatile" => TokenKind::Keyword(Keyword::Volatile),
                "void" => TokenKind::Keyword(Keyword::Void),
                "signed" => TokenKind::Keyword(Keyword::Signed),
                "unsigned" => TokenKind::Keyword(Keyword::Unsigned),
                "char" => TokenKind::Keyword(Keyword::Char),
                "int" => TokenKind::Keyword(Keyword::Int),
                "bool" => TokenKind::Keyword(Keyword::Int),
                "short" => TokenKind::Keyword(Keyword::Short),
                "long" => TokenKind::Keyword(Keyword::Long),
                "float" => TokenKind::Keyword(Keyword::Float),
                "double" => TokenKind::Keyword(Keyword::Double),
                "struct" => TokenKind::Keyword(Keyword::Struct),
                "union" => TokenKind::Keyword(Keyword::Union),
                "enum" => TokenKind::Keyword(Keyword::Enum),
                "inline" => TokenKind::Keyword(Keyword::Inline),
                "noreturn" => TokenKind::Keyword(Keyword::Noreturn),
                "if" => TokenKind::Keyword(Keyword::If),
                "else" => TokenKind::Keyword(Keyword::Else),
                "for" => TokenKind::Keyword(Keyword::For),
                "while" => TokenKind::Keyword(Keyword::While),
                "do" => TokenKind::Keyword(Keyword::Do),
                "switch" => TokenKind::Keyword(Keyword::Switch),
                "case" => TokenKind::Keyword(Keyword::Case),
                "default" => TokenKind::Keyword(Keyword::Default),
                "goto" => TokenKind::Keyword(Keyword::Goto),
                "break" => TokenKind::Keyword(Keyword::Break),
                "continue" => TokenKind::Keyword(Keyword::Continue),
                "return" => TokenKind::Keyword(Keyword::Return),
                _ => return token,
            };
            return Token::new(keyw, 0, pos, line);
        }
        token
    }

    fn expand_obj_macro(
        &mut self,
        token: Token,
        name: String,
        macro_body: &Vec<Token>,
    ) -> ParseR<()> {
        let body = macro_body
            .iter()
            .map(|tok| {
                let mut t = tok.clone();
                t.add_hideset(name.to_string());
                t.pos = token.pos.clone();
                t
            })
            .collect();
        self.unget_all(&body);
        Ok(())
    }
    fn read_one_arg(&mut self, end: &mut bool) -> ParseR<Vec<Token>> {
        let mut nest = 0;
        let mut arg = Vec::new();
        loop {
            let tok = self.do_read_token()?;
            let val = ident_val!(tok);
            if nest == 0 {
                match val.as_str() {
                    ")" => {
                        *end = true;
                        break;
                    }
                    "," => break,
                    _ => {}
                }
            }
            match val.as_str() {
                "(" => nest += 1,
                ")" => nest -= 1,
                _ => {}
            }
            arg.push(tok);
        }
        Ok(arg)
    }
    fn stringize(&mut self, pos: &Pos, tokens: &Vec<Token>) -> Token {
        let string = tokens
            .iter()
            .map(|token| {
                format!(
                    "{}{}",
                    if token.space { " " } else { "" },
                    match token.kind {
                        TokenKind::String(ref s) => format!("\"{}\"", s.as_str()),
                        TokenKind::IntNumber(ref i, _) => format!("{}", *i),
                        TokenKind::FloatNumber(ref f) => format!("{}", *f),
                        TokenKind::Identifier(ref i) => format!("{}", *i),
                        TokenKind::Char(ref c) => format!("\'{}\'", *c),
                        _ => "".to_string(),
                    }
                )
            })
            .fold("".to_string(), |a, s| a + s.as_str())
            .trim_start() // remove leading spaces
            .to_string();
        Token::new(TokenKind::String(string), 0, pos.pos, pos.line)
    }
    fn expand_func_macro(
        &mut self,
        token: Token,
        name: String,
        macro_body: &Vec<Token>,
    ) -> ParseR<()> {
        // expect '(', self.skip can't be used because self.skip uses 'self.get' that uses MACRO_MAP with Mutex
        let expect_bracket = self.read_token()?;
        if expect_bracket.kind != TokenKind::Symbol(Symbol::OpeningParen) {
            error::error_exit(*self.get_cur_line() as i32, "expected '('");
        }

        let mut args = Vec::new();
        let mut end = false;
        while !end {
            args.push(self.read_one_arg(&mut end)?);
        }

        let mut expanded = Vec::new();
        let mut is_stringize = false;
        let mut is_combine = false;
        // TODO: refine code
        for macro_tok in macro_body {
            if ident_val!(macro_tok) == "#" {
                if is_stringize {
                    // means ##
                    is_stringize = false;
                    is_combine = true;
                } else {
                    is_stringize = true;
                }
                continue;
            }

            if macro_tok.kind == TokenKind::MacroParam {
                let position = macro_tok.macro_position;

                if is_stringize {
                    let stringized = self.stringize(&token.pos, &args[position]);
                    expanded.push(stringized);
                    is_stringize = false;
                } else if is_combine {
                    let mut last = expanded.pop().unwrap();
                    for t in &args[position] {
                        *ident_mut_val!(last) += ident_val!(t).as_str();
                    }
                    expanded.push(last);
                    is_combine = false;
                } else {
                    self.buf.push_back(VecDeque::new());
                    self.unget_all(&args[position]);
                    loop {
                        match self.get_token() {
                            Ok(ok) => expanded.push(ok),
                            Err(Error::EOF) => break,
                            Err(e) => return Err(e),
                        }
                    }
                    self.buf.pop_back();
                }
            } else {
                if is_combine {
                    let mut last = expanded.pop().unwrap();
                    *ident_mut_val!(last) += ident_val!(macro_tok).as_str();
                    expanded.push(last);
                } else {
                    expanded.push(macro_tok.clone());
                }
            }
        }

        for tok in &mut expanded {
            tok.add_hideset(name.to_string());
            tok.pos = token.pos.clone();
        }

        self.unget_all(&expanded);
        Ok(())
    }
    fn expand(&mut self, token: ParseR<Token>) -> ParseR<Token> {
        token.and_then(|tok| {
            let name = ident_val!(tok);
            match name.as_str() {
                "__LINE__" => {
                    return Ok(Token::new(
                        TokenKind::IntNumber(*self.get_cur_line() as i64, Bits::Bits32),
                        0,
                        tok.pos.pos,
                        tok.pos.line,
                    ))
                }
                "__FILE__" => {
                    return Ok(Token::new(
                        TokenKind::String(self.get_filename()),
                        0,
                        tok.pos.pos,
                        tok.pos.line,
                    ))
                }
                _ => {}
            }
            if tok.hideset.contains(name.as_str()) || !self.macro_map.contains_key(name.as_str()) {
                Ok(tok)
            } else {
                // if cur token is macro:
                match self.macro_map.get(name.as_str()).unwrap().clone() {
                    Macro::Object(ref body) => self.expand_obj_macro(tok, name, body),
                    Macro::FuncLike(ref body) => self.expand_func_macro(tok, name, body),
                }?;
                self.get_token()
            }
        })
    }

    fn get_token(&mut self) -> ParseR<Token> {
        let tok = self.read_token().and_then(|tok| match &tok.kind {
            &TokenKind::Symbol(Symbol::Hash) => {
                self.read_cpp_directive()?;
                self.get_token()
            }
            _ => Ok(tok),
        });
        self.expand(tok)
    }

    pub fn get(&mut self) -> ParseR<Token> {
        self.get_token().and_then(|tok| {
            if matches!(tok.kind, TokenKind::String(_))
                && matches!(self.peek()?.kind, TokenKind::String(_))
            {
                let s1 = retrieve_str!(tok);
                let s2 = retrieve_str!(self.get()?);
                let mut new_tok = tok;
                let mut concat_str = s1;
                concat_str.push_str(s2.as_str());
                new_tok.kind = TokenKind::String(concat_str);
                Ok(new_tok)
            } else {
                Ok(self.maybe_convert_to_keyword(tok))
            }
        })
    }

    pub fn peek(&mut self) -> ParseR<Token> {
        self.get_token().and_then(|tok| {
            let conv = self.maybe_convert_to_keyword(tok);
            self.unget(conv.clone());
            Ok(conv)
        })
    }

    // for c preprocessor

    fn read_cpp_directive(&mut self) -> ParseR<()> {
        let tok = self.do_read_token(); // cpp directive
        tok.and_then(|t| match ident_val!(t).as_str() {
            "include" => self.read_include(),
            "define" => self.read_define(),
            "undef" => self.read_undef(),
            "if" => self.read_if(),
            "ifdef" => self.read_ifdef(),
            "ifndef" => self.read_ifndef(),
            "elif" => self.read_elif(),
            "else" => self.read_else(),
            _ => Ok(()),
        })
    }

    fn try_include(&mut self, filename: &str) -> Option<String> {
        let header_paths = vec![
            "./include/",
            "/include/",
            "/usr/include/",
            "/usr/include/linux/",
            "/usr/include/x86_64-linux-gnu/",
            "./include/",
            "",
        ];
        header_paths
            .iter()
            .find(|header_path| {
                let abs_filename = format!("{}{}", header_path, filename);
                path::Path::new(abs_filename.as_str()).exists()
            })
            .and_then(|a| Some(a.to_string() + filename))
    }
    fn read_headerfile_name(&mut self) -> ParseR<String> {
        let mut name = "".to_string();
        // Lt = '<'
        if self.skip_symbol(Symbol::Lt)? {
            while !self.peek_char_is('>')? {
                name.push(self.peek_next()?);
            }
            self.peek_next()?; // >
        } else {
            let tok = self.do_read_token()?;
            if let TokenKind::String(s) = tok.kind {
                println!("sorry, using \"double quote\" in #include is currently not supported.");
                name = s;
            } else {
                error::error_exit(*self.get_cur_line() as i32, "expected '<' or '\"'");
            }
        }
        Ok(name)
    }
    fn read_include(&mut self) -> ParseR<()> {
        // this will be a function
        let filename = self.read_headerfile_name()?;
        let abs_filename = self
            .try_include(filename.as_str())
            .or_else(|| {
                println!("error: {}: not found '{}'", *self.get_cur_line(), filename);
                process::exit(-1);
            })
            .unwrap();
        // DEBUG: println!("include filename: {}", abs_filename);

        let mut include_file = OpenOptions::new()
            .read(true)
            .open(abs_filename.to_string())
            .unwrap();
        let mut body = String::with_capacity(512);
        include_file
            .read_to_string(&mut body)
            .ok()
            .expect("not found file");
        self.filename.push_back(abs_filename);
        unsafe {
            self.peek.push_back(body.as_mut_vec().clone());
        }
        self.peek_pos.push_back(0);
        self.cur_line.push_back(1);
        Ok(())
    }

    fn read_define_obj_macro(&mut self, name: String) -> ParseR<()> {
        // DEBUG: println!("\tmacro: {}", name);

        let mut body = Vec::new();
        // DEBUG: print!("\tmacro body: ");
        loop {
            let c = self.do_read_token()?;
            if c.kind == TokenKind::Newline {
                break;
            }
            // DEBUG: print!("{}{}", if c.space { " " } else { "" }, c.val);
            body.push(c);
        }
        // DEBUG: println!();
        self.register_obj_macro(name, body);
        Ok(())
    }
    fn read_define_func_macro(&mut self, name: String) -> ParseR<()> {
        // read macro arguments
        let mut params = HashMap::new();
        let mut count = 0usize;
        loop {
            let mut arg = ident_val!(self.do_read_token()?);
            if arg == ")" {
                break;
            }
            if count > 0 {
                if arg != "," {
                    error::error_exit(*self.get_cur_line() as i32, "expected comma");
                }
                arg = ident_val!(self.do_read_token()?);
            }
            params.insert(arg, count);
            count += 1;
        }

        let mut body = Vec::new();
        // print!("\tmacro body: ");
        loop {
            let tok = self.do_read_token()?;
            if tok.kind == TokenKind::Newline {
                break;
            }

            // if tok is a parameter of funclike macro,
            //  the kind of tok will be changed to MacroParam
            //  and set macro_position
            let maybe_macro_name = ident_val!(tok);
            // print!("{}{}", if tok.space { " " } else { "" }, tok.val);
            if params.contains_key(maybe_macro_name.as_str()) {
                let mut macro_param = tok;
                macro_param.kind = TokenKind::MacroParam;
                macro_param.macro_position = *params.get(maybe_macro_name.as_str()).unwrap();
                body.push(macro_param);
            } else {
                body.push(tok);
            }
        }
        self.register_funclike_macro(name, body);
        Ok(())
    }
    fn read_define(&mut self) -> ParseR<()> {
        let mcro = self.do_read_token()?;
        assert!(matches!(mcro.kind, TokenKind::Identifier(_)));
        // println!("define: {}", mcro.val);

        let t = self.do_read_token()?;
        if !t.space && ident_val!(t).as_str() == "(" {
            self.read_define_func_macro(ident_val!(mcro))
        } else {
            self.unget(t);
            self.read_define_obj_macro(ident_val!(mcro))
        }
    }
    fn read_undef(&mut self) -> ParseR<()> {
        let mcro = self.do_read_token()?;
        assert!(matches!(mcro.kind, TokenKind::Identifier(_)));
        self.macro_map.remove(ident_val!(mcro).as_str());
        Ok(())
    }

    fn register_obj_macro(&mut self, name: String, body: Vec<Token>) {
        self.macro_map.insert(name, Macro::Object(body));
    }
    fn register_funclike_macro(&mut self, name: String, body: Vec<Token>) {
        self.macro_map.insert(name, Macro::FuncLike(body));
    }

    fn read_defined_op(&mut self) -> ParseR<Token> {
        let mut tok = self.do_read_token()?;
        if ident_val!(tok) == "(" {
            tok = self.do_read_token()?;
            self.expect_skip_symbol(Symbol::ClosingParen)?;
        }
        if self.macro_map.contains_key(ident_val!(tok).as_str()) {
            Ok(Token::new(
                TokenKind::IntNumber(1, Bits::Bits32),
                0,
                0,
                *self.get_cur_line(),
            ))
        } else {
            Ok(Token::new(
                TokenKind::IntNumber(0, Bits::Bits32),
                0,
                0,
                *self.get_cur_line(),
            ))
        }
    }
    fn read_intexpr_line(&mut self) -> ParseR<Vec<Token>> {
        let mut v = Vec::new();
        loop {
            let mut tok = self.do_read_token()?;
            tok = self.expand(Ok(tok))?;
            if tok.kind == TokenKind::Newline {
                break;
            }

            tok = self.convert_to_symbol(tok);
            match tok.kind {
                TokenKind::Identifier(ident) => {
                    if ident == "defined" {
                        v.push(self.read_defined_op()?);
                    } else {
                        // identifier in expr line is replaced with 0i
                        v.push(Token::new(
                            TokenKind::IntNumber(0, Bits::Bits32),
                            0,
                            0,
                            *self.get_cur_line(),
                        ));
                    }
                }
                _ => v.push(tok),
            }
        }
        Ok(v)
    }
    fn read_constexpr(&mut self) -> ParseR<bool> {
        let expr_line = self.read_intexpr_line()?;
        self.buf.push_back(VecDeque::new());

        self.unget(Token::new(TokenKind::Symbol(Symbol::Semicolon), 0, 0, 0));
        self.unget_all(&expr_line);

        let node = parser::Parser::new(self).run_as_expr().ok().unwrap();

        self.buf.pop_back();

        if let Ok(e) = node.eval_constexpr() {
            Ok(e != 0)
        } else {
            println!("error: lexer constexpr");
            Err(Error::Something)
        }
    }

    fn do_read_if(&mut self, cond: bool) -> ParseR<()> {
        self.cond_stack.push(cond);
        if !cond {
            self.skip_cond_include()?;
        }
        Ok(())
    }
    fn read_if(&mut self) -> ParseR<()> {
        let cond = self.read_constexpr()?;
        self.do_read_if(cond)
    }
    fn read_ifdef(&mut self) -> ParseR<()> {
        let macro_name = ident_val!(self.do_read_token()?);
        let macro_is_defined = self.macro_map.contains_key(macro_name.as_str());
        self.do_read_if(macro_is_defined)
    }
    fn read_ifndef(&mut self) -> ParseR<()> {
        let macro_name = ident_val!(self.do_read_token()?);
        let macro_is_undefined = !self.macro_map.contains_key(macro_name.as_str());
        self.do_read_if(macro_is_undefined)
    }
    fn read_elif(&mut self) -> ParseR<()> {
        if *self.cond_stack.last().unwrap() || !self.read_constexpr()? {
            self.skip_cond_include()?;
        } else {
            self.cond_stack.pop();
            self.cond_stack.push(true);
        }
        Ok(())
    }
    fn read_else(&mut self) -> ParseR<()> {
        if *self.cond_stack.last().unwrap() {
            self.skip_cond_include()?;
        }
        Ok(())
    }

    fn skip_cond_include(&mut self) -> ParseR<()> {
        let mut nest = 0;
        loop {
            if self.peek_next()? != '#' {
                continue;
            }

            let tok = self.do_read_token()?;
            let val = ident_val!(tok);
            if nest == 0 {
                match val.as_str() {
                    "else" | "elif" | "endif" => {
                        let line = *self.get_cur_line();
                        self.unget(tok);
                        self.unget(Token::new(
                            TokenKind::Identifier("#".to_string()),
                            0,
                            0,
                            line,
                        ));
                        return Ok(());
                    }
                    _ => {}
                }
            }

            match val.as_str() {
                "if" | "ifdef" | "ifndef" => nest += 1,
                "endif" => nest -= 1,
                _ => {}
            }
        }
    }

    pub fn get_surrounding_code_with_err_point(&mut self, pos: usize) -> String {
        let code = self.peek.back().unwrap();
        let peek_pos = pos;
        let start_pos = {
            let mut p = peek_pos as i32;
            while p >= 0 && code[p as usize] as char != '\n' {
                p -= 1;
            }
            p += 1; // '\n'
            p as usize
        };
        let end_pos = {
            let mut p = peek_pos as i32;
            while p < code.len() as i32 && code[p as usize] as char != '\n' {
                p += 1;
            }
            p as usize
        };
        let surrounding_code = String::from_utf8(code[start_pos..end_pos].to_vec())
            .unwrap()
            .to_string();
        let mut err_point = String::new();
        for _ in 0..(peek_pos - start_pos) {
            err_point.push(' ');
        }
        err_point.push('^');
        surrounding_code + "\n" + err_point.as_str()
    }
}
