use std::fs::OpenOptions;
use std::io::prelude::*;
use std::str;
use std::collections::VecDeque;
use std::path;
use std::process;
use std::collections::{HashSet, HashMap};
use error;
use parser;

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
    While,
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
pub enum TokenKind {
    MacroParam,
    Keyword(Keyword),
    Identifier,
    IntNumber(i64),
    FloatNumber(f64),
    String(String),
    Char,
    Symbol(Symbol),
    Newline,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub space: bool, // leading space
    pub val: String,
    pub macro_position: usize,
    pub hideset: HashSet<String>,
    pub pos: usize,
    pub line: i32,
}

impl Token {
    pub fn new(kind: TokenKind, val: &str, macro_position: usize, pos: usize, line: i32) -> Token {
        Token {
            kind: kind,
            space: false,
            val: val.to_string(),
            macro_position: macro_position,
            hideset: HashSet::new(),
            pos: pos,
            line: line,
        }
    }
}

#[derive(Clone)]
pub struct Lexer {
    pub cur_line: VecDeque<i32>,
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

        let mut file = OpenOptions::new()
            .read(true)
            .open(filename.to_string())
            .unwrap();
        let mut file_body = String::new();
        file.read_to_string(&mut file_body)
            .ok()
            .expect("cannot open file");

        let mut rucc_header = OpenOptions::new()
            .read(true)
            .open("./include/rucc.h")
            .unwrap();
        let mut rucc_header_body = String::new();
        rucc_header
            .read_to_string(&mut rucc_header_body)
            .ok()
            .expect("cannot open file");
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
    pub fn get_filename(self) -> String {
        self.filename.back().unwrap().to_owned()
    }
    fn peek_get(&mut self) -> Option<char> {
        let peek = self.peek.back_mut().unwrap();
        let peek_pos = *self.peek_pos.back_mut().unwrap();
        if peek_pos >= peek.len() {
            None
        } else {
            Some(peek[peek_pos] as char)
        }
    }
    fn peek_next(&mut self) -> char {
        let peek_pos = self.peek_pos.back_mut().unwrap();
        let ret = self.peek.back_mut().unwrap()[*peek_pos] as char;
        *peek_pos += 1;
        ret
    }
    fn peek_next_char_is(&mut self, ch: char) -> bool {
        let nextc = self.peek.back_mut().unwrap()[*self.peek_pos.back_mut().unwrap() + 1] as char;
        nextc == ch
    }
    fn peek_char_is(&mut self, ch: char) -> bool {
        let line = *self.cur_line.back_mut().unwrap();
        let errf =
            || -> Option<char> { error::error_exit(line, format!("expected '{}'", ch).as_str()); };
        let peekc = self.peek_get().or_else(errf).unwrap();
        peekc == ch
    }

    pub fn peek_token_is(&mut self, expect: &str) -> bool {
        let peek = self.peek_e();
        peek.val == expect && peek.kind != TokenKind::Char
    }
    pub fn peek_keyword_token_is(&mut self, expect: Keyword) -> bool {
        let peek = self.peek_e();
        peek.kind == TokenKind::Keyword(expect)
    }
    pub fn peek_symbol_token_is(&mut self, expect: Symbol) -> bool {
        let peek = self.peek_e();
        peek.kind == TokenKind::Symbol(expect)
    }
    pub fn next_token_is(&mut self, expect: &str) -> bool {
        let peek = self.get_e();
        let next = self.get_e();
        let next_token_is_expect = next.val == expect && next.kind != TokenKind::Char;
        self.unget(next);
        self.unget(peek);
        next_token_is_expect
    }
    pub fn next_keyword_token_is(&mut self, expect: Keyword) -> bool {
        let peek = self.get_e();
        let next = self.get_e();
        let next_token_is_expect = next.kind == TokenKind::Keyword(expect);
        self.unget(next);
        self.unget(peek);
        next_token_is_expect
    }
    pub fn next_symbol_token_is(&mut self, expect: Symbol) -> bool {
        let peek = self.get_e();
        let next = self.get_e();
        let next_token_is_expect = next.kind == TokenKind::Symbol(expect);
        self.unget(next);
        self.unget(peek);
        next_token_is_expect
    }
    pub fn skip_keyword(&mut self, keyword: Keyword) -> bool {
        match self.get() {
            Some(n) => {
                if n.kind == TokenKind::Keyword(keyword) {
                    true
                } else {
                    self.buf.back_mut().unwrap().push_back(n);
                    false
                }
            }
            None => false,
        }
    }
    pub fn skip_symbol(&mut self, sym: Symbol) -> bool {
        match self.get() {
            Some(tok) => {
                if tok.kind == TokenKind::Symbol(sym) {
                    true
                } else {
                    self.buf.back_mut().unwrap().push_back(tok);
                    false
                }
            }
            None => false,
        }
    }
    pub fn expect_skip_keyword(&mut self, expect: Keyword) -> bool {
        if !self.skip_keyword(expect.clone()) {
            error::error_exit(*self.cur_line.back_mut().unwrap(),
                              format!("expected the keyword '{:?}'", expect).as_str());
        }
        true
    }
    pub fn expect_skip_symbol(&mut self, expect: Symbol) -> bool {
        if !self.skip_symbol(expect.clone()) {
            error::error_exit(*self.cur_line.back_mut().unwrap(),
                              format!("expected the keyword '{:?}'", expect).as_str());
        }
        true
    }
    pub fn unget(&mut self, t: Token) {
        self.buf.back_mut().unwrap().push_back(t);
    }
    pub fn unget_all(&mut self, mut tv: Vec<Token>) {
        tv.reverse();
        for t in tv {
            self.unget(t);
        }
    }

    pub fn read_identifier(&mut self) -> Token {
        let mut ident = String::with_capacity(16);
        let pos = *self.peek_pos.back().unwrap();
        loop {
            let c = self.peek_next();
            match c {
                'a'...'z' | 'A'...'Z' | '_' | '0'...'9' => ident.push(c),
                _ => break,
            };
        }
        *self.peek_pos.back_mut().unwrap() -= 1;
        Token::new(TokenKind::Identifier,
                   ident.as_str(),
                   0,
                   pos,
                   *self.cur_line.back_mut().unwrap())
    }
    fn read_number_literal(&mut self) -> Token {
        let mut num = String::with_capacity(8);
        let mut is_float = false;
        let pos = *self.peek_pos.back().unwrap();
        loop {
            match self.peek_get() {
                Some(c) => {
                    match c {
                        '.' | '0'...'9' | 'a'...'z' | 'A'...'Z' => {
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
            let f: f64 = num.parse().unwrap();
            Token::new(TokenKind::FloatNumber(f),
                       "",
                       0,
                       pos,
                       *self.cur_line.back_mut().unwrap())
        } else {
            let i = if num.len() > 2 && num.chars().nth(1).unwrap() == 'x' {
                self.read_hex_num(&num[2..])
            } else if num.chars().nth(0).unwrap() == '0' {
                self.read_oct_num(&num[1..])
            } else {
                self.read_dec_num(num.as_str())
            };
            Token::new(TokenKind::IntNumber(i),
                       "",
                       0,
                       pos,
                       *self.cur_line.back_mut().unwrap())
        }
    }
    fn read_dec_num(&mut self, num_literal: &str) -> i64 {
        let mut n = 0i64;
        for c in num_literal.chars() {
            match c {
                '0'...'9' => n = n * 10 + c.to_digit(10).unwrap() as i64,
                _ => {} // TODO: suffix
            }
        }
        n
    }
    fn read_oct_num(&mut self, num_literal: &str) -> i64 {
        let mut n = 0i64;
        for c in num_literal.chars() {
            match c {
                '0'...'7' => n = n * 8 + c.to_digit(8).unwrap() as i64,
                _ => {} // TODO: suffix
            }
        }
        n
    }
    fn read_hex_num(&mut self, num_literal: &str) -> i64 {
        let mut n = 0u64;
        for c in num_literal.chars() {
            match c {
                '0'...'9' | 'A'...'F' | 'a'...'f' => n = n * 16 + c.to_digit(16).unwrap() as u64,
                _ => {} // TODO: suffix
            }
        }
        n as i64
    }
    pub fn read_newline(&mut self) -> Token {
        let pos = *self.peek_pos.back().unwrap();
        self.peek_next();
        *self.cur_line.back_mut().unwrap() += 1;
        Token::new(TokenKind::Newline,
                   "",
                   0,
                   pos,
                   *self.cur_line.back_mut().unwrap())
    }
    pub fn read_symbol(&mut self) -> Token {
        let pos = *self.peek_pos.back().unwrap();
        let c = self.peek_next();
        let mut sym = String::new();
        sym.push(c);
        match c {
            '+' | '-' => {
                if self.peek_char_is('=') || self.peek_char_is('>') || self.peek_char_is('+') ||
                   self.peek_char_is('-') {
                    sym.push(self.peek_next());
                }
            }
            '*' | '/' | '%' | '=' | '^' | '!' => {
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
        Token::new(TokenKind::Identifier,
                   sym.as_str(),
                   0,
                   pos,
                   *self.cur_line.back_mut().unwrap())
    }
    fn read_escaped_char(&mut self) -> char {
        let c = self.peek_next();
        match c {
            '\'' | '"' | '?' | '\\' => c,
            'a' => '\x07',
            'b' => '\x08',
            'f' => '\x0c',
            'n' => '\x0a',
            'r' => '\x0d',
            't' => '\x09',
            'v' => '\x0b',
            'x' => {
                loop {
                    match self.peek_get().unwrap() {
                        '0'...'9' | 'a'...'f' | 'A'...'F' => {}
                        _ => break,
                    }
                    self.peek_next();
                }
                'A'
            }
            _ => c,
        }
    }
    fn read_string_literal(&mut self) -> Token {
        let pos = *self.peek_pos.back().unwrap();
        self.peek_next(); // '"'
        let mut s = String::new();
        loop {
            let c = self.peek_next();
            match c {
                '"' => break,
                '\\' => s.push(self.read_escaped_char()),
                _ => s.push(c),
            }
        }
        Token::new(TokenKind::String(s),
                   "",
                   0,
                   pos,
                   *self.cur_line.back_mut().unwrap())
    }
    fn read_char_literal(&mut self) -> Token {
        let pos = *self.peek_pos.back().unwrap();
        self.peek_next(); // '\''
        let c = self.peek_next();
        let mut s = String::new();
        s.push(if c == '\\' {
                   self.read_escaped_char()
               } else {
                   c
               });
        if self.peek_next() != '\'' {
            error::error_exit(*self.cur_line.back().unwrap(),
                              "missing terminating \' char");
        }
        Token::new(TokenKind::Char,
                   s.as_str(),
                   0,
                   pos,
                   *self.cur_line.back_mut().unwrap())
    }

    pub fn do_read_token(&mut self) -> Option<Token> {
        if !self.buf.back_mut().unwrap().is_empty() {
            return self.buf.back_mut().unwrap().pop_back();
        }

        match self.peek_get() {
            Some(c) => {
                match c {
                    'a'...'z' | 'A'...'Z' | '_' => Some(self.read_identifier()),
                    ' ' | '\t' => {
                        self.peek_next();
                        self.do_read_token()
                        // set a leading space
                            .and_then(|tok| {
                                          let mut t = tok;
                                          t.space = true;
                                          Some(t)
                                      })
                    }
                    '0'...'9' => Some(self.read_number_literal()),
                    '\"' => Some(self.read_string_literal()),
                    '\'' => Some(self.read_char_literal()),
                    '\n' => Some(self.read_newline()),
                    '\\' => {
                        while self.peek_next() != '\n' {}
                        self.do_read_token()
                    }
                    '/' => {
                        if self.peek_next_char_is('*') {
                            self.peek_next(); // /
                            self.peek_next(); // *
                            while !(self.peek_char_is('*') && self.peek_next_char_is('/')) {
                                self.peek_next();
                            }
                            self.peek_next(); // *
                            self.peek_next(); // /
                            self.do_read_token()
                        } else if self.peek_next_char_is('/') {
                            self.peek_next(); // /
                            self.peek_next(); // /
                            while !self.peek_char_is('\n') {
                                self.peek_next();
                            }
                            // self.peek_next(); // \n
                            self.do_read_token()
                        } else {
                            Some(self.read_symbol())
                        }
                    }
                    _ => Some(self.read_symbol()),
                }
            }
            None => {
                if self.peek.len() > 1 {
                    self.peek.pop_back();
                    self.peek_pos.pop_back();
                    self.filename.pop_back();
                    self.cur_line.pop_back();
                    self.do_read_token()
                } else {
                    None as Option<Token>
                }
            }
        }
    }
    pub fn read_token(&mut self) -> Option<Token> {
        let token = self.do_read_token();
        token.and_then(|tok| match tok.kind {
                           TokenKind::Newline => self.read_token(),
                           TokenKind::Identifier => Some(self.convert_to_keyword_or_symbol(tok)),
                           _ => Some(tok),
                       })
    }
    fn convert_to_keyword_or_symbol(&mut self, token: Token) -> Token {
        let pos = token.pos;
        match token.val.as_str() {
            "typedef" => {
                Token::new(TokenKind::Keyword(Keyword::Typedef),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "extern" => {
                Token::new(TokenKind::Keyword(Keyword::Extern),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "auto" => {
                Token::new(TokenKind::Keyword(Keyword::Auto),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "register" => {
                Token::new(TokenKind::Keyword(Keyword::Register),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "static" => {
                Token::new(TokenKind::Keyword(Keyword::Static),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "restrict" => {
                Token::new(TokenKind::Keyword(Keyword::Restrict),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "const" => {
                Token::new(TokenKind::Keyword(Keyword::Const),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "volatile" => {
                Token::new(TokenKind::Keyword(Keyword::Volatile),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "void" => {
                Token::new(TokenKind::Keyword(Keyword::Void),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "signed" => {
                Token::new(TokenKind::Keyword(Keyword::Signed),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "unsigned" => {
                Token::new(TokenKind::Keyword(Keyword::Unsigned),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "char" => {
                Token::new(TokenKind::Keyword(Keyword::Char),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "int" => {
                Token::new(TokenKind::Keyword(Keyword::Int),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "short" => {
                Token::new(TokenKind::Keyword(Keyword::Short),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "long" => {
                Token::new(TokenKind::Keyword(Keyword::Long),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "float" => {
                Token::new(TokenKind::Keyword(Keyword::Float),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "double" => {
                Token::new(TokenKind::Keyword(Keyword::Double),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "struct" => {
                Token::new(TokenKind::Keyword(Keyword::Struct),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "union" => {
                Token::new(TokenKind::Keyword(Keyword::Union),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "enum" => {
                Token::new(TokenKind::Keyword(Keyword::Enum),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "inline" => {
                Token::new(TokenKind::Keyword(Keyword::Inline),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "noreturn" => {
                Token::new(TokenKind::Keyword(Keyword::Noreturn),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "if" => {
                Token::new(TokenKind::Keyword(Keyword::If),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "else" => {
                Token::new(TokenKind::Keyword(Keyword::Else),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "for" => {
                Token::new(TokenKind::Keyword(Keyword::For),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "while" => {
                Token::new(TokenKind::Keyword(Keyword::While),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "return" => {
                Token::new(TokenKind::Keyword(Keyword::Return),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "++" => {
                Token::new(TokenKind::Symbol(Symbol::Inc),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            } 
            "--" => {
                Token::new(TokenKind::Symbol(Symbol::Dec),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "(" => {
                Token::new(TokenKind::Symbol(Symbol::OpeningParen),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            ")" => {
                Token::new(TokenKind::Symbol(Symbol::ClosingParen),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "[" => {
                Token::new(TokenKind::Symbol(Symbol::OpeningBoxBracket),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "]" => {
                Token::new(TokenKind::Symbol(Symbol::ClosingBoxBracket),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "{" => {
                Token::new(TokenKind::Symbol(Symbol::OpeningBrace),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "}" => {
                Token::new(TokenKind::Symbol(Symbol::ClosingBrace),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "." => {
                Token::new(TokenKind::Symbol(Symbol::Point),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "," => {
                Token::new(TokenKind::Symbol(Symbol::Comma),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            ";" => {
                Token::new(TokenKind::Symbol(Symbol::Semicolon),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            ":" => {
                Token::new(TokenKind::Symbol(Symbol::Colon),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "->" => {
                Token::new(TokenKind::Symbol(Symbol::Arrow),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "+" => {
                Token::new(TokenKind::Symbol(Symbol::Add),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "-" => {
                Token::new(TokenKind::Symbol(Symbol::Sub),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "!" => {
                Token::new(TokenKind::Symbol(Symbol::Not),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "~" => {
                Token::new(TokenKind::Symbol(Symbol::BitwiseNot),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "*" => {
                Token::new(TokenKind::Symbol(Symbol::Asterisk),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "&" => {
                Token::new(TokenKind::Symbol(Symbol::Ampersand),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "/" => {
                Token::new(TokenKind::Symbol(Symbol::Div),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "%" => {
                Token::new(TokenKind::Symbol(Symbol::Mod),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "<<" => {
                Token::new(TokenKind::Symbol(Symbol::Shl),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            ">>" => {
                Token::new(TokenKind::Symbol(Symbol::Shr),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "<" => {
                Token::new(TokenKind::Symbol(Symbol::Lt),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "<=" => {
                Token::new(TokenKind::Symbol(Symbol::Le),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            ">" => {
                Token::new(TokenKind::Symbol(Symbol::Gt),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            ">=" => {
                Token::new(TokenKind::Symbol(Symbol::Ge),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "==" => {
                Token::new(TokenKind::Symbol(Symbol::Eq),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "!=" => {
                Token::new(TokenKind::Symbol(Symbol::Ne),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "^" => {
                Token::new(TokenKind::Symbol(Symbol::Xor),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "|" => {
                Token::new(TokenKind::Symbol(Symbol::Or),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "&&" => {
                Token::new(TokenKind::Symbol(Symbol::LAnd),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "||" => {
                Token::new(TokenKind::Symbol(Symbol::LOr),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "?" => {
                Token::new(TokenKind::Symbol(Symbol::Question),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "=" => {
                Token::new(TokenKind::Symbol(Symbol::Assign),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "+=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignAdd),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "-=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignSub),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "*=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignMul),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "/=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignDiv),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "%=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignMod),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "<<=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignShl),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            ">>=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignShr),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "&=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignAnd),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "^=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignXor),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "|=" => {
                Token::new(TokenKind::Symbol(Symbol::AssignOr),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "#" => {
                Token::new(TokenKind::Symbol(Symbol::Hash),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "..." => {
                Token::new(TokenKind::Symbol(Symbol::Vararg),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            "sizeof" => {
                Token::new(TokenKind::Symbol(Symbol::Sizeof),
                           "",
                           0,
                           pos,
                           *self.cur_line.back_mut().unwrap())
            }
            _ => token,
        }
    }

    fn expand_obj_macro(&mut self, name: String, macro_body: &Vec<Token>) {
        let mut body: Vec<Token> = Vec::new();
        for tok in macro_body {
            body.push(|| -> Token {
                          let mut t = (*tok).clone();
                          t.hideset.insert(name.to_string());
                          t
                      }());
        }
        self.unget_all(body);
    }
    fn read_one_arg(&mut self) -> Vec<Token> {
        let mut n = 0;
        let mut arg: Vec<Token> = Vec::new();
        loop {
            let tok = self.do_read_token()
                .or_else(|| {
                             error::error_exit(*self.cur_line.back_mut().unwrap(),
                                               "expected macro args but reach EOF")
                         })
                .unwrap();
            if n == 0 {
                if tok.val == ")" {
                    self.unget(tok);
                    break;
                } else if tok.val == "," {
                    break;
                }
            }
            match tok.val.as_str() {
                "(" => n += 1,
                ")" => n -= 1,
                _ => {}
            }
            arg.push(tok);
        }
        arg
    }
    fn stringize(&mut self, tokens: &Vec<Token>) -> Token {
        let mut string = String::new();
        for token in tokens {
            string += format!("{}{}",
                              (if token.space { " " } else { "" }),
                              match token.kind {
                                  TokenKind::String(ref s) => format!("\"{}\"", s.as_str()),
                                  TokenKind::IntNumber(ref i) => format!("{}", *i),
                                  TokenKind::FloatNumber(ref f) => format!("{}", *f),
                                  _ => token.val.to_string(),
                              })
                    .as_str();
        }
        Token::new(TokenKind::String(string),
                   "",
                   0,
                   0,
                   *self.cur_line.back_mut().unwrap())
    }
    fn expand_func_macro(&mut self, name: String, macro_body: &Vec<Token>) {
        // expect '(', (self.skip can't be used because skip uses 'self.get' that uses MACRO_MAP using Mutex
        let expect_bracket = self.read_token()
            .or_else(|| {
                         error::error_exit(*self.cur_line.back_mut().unwrap(),
                                           "expected '(' but reach EOF")
                     })
            .unwrap();
        if expect_bracket.kind != TokenKind::Symbol(Symbol::OpeningParen) {
            error::error_exit(*self.cur_line.back_mut().unwrap(), "expected '('");
        }

        let mut args: Vec<Vec<Token>> = Vec::new();
        // read macro arguments
        loop {
            let maybe_bracket = self.do_read_token()
                .or_else(|| {
                             error::error_exit(*self.cur_line.back_mut().unwrap(),
                                               "expected ')' but reach EOF")
                         })
                .unwrap();
            if maybe_bracket.val == ")" {
                break;
            } else {
                self.unget(maybe_bracket);
            }
            args.push(self.read_one_arg());
        }

        let mut expanded: Vec<Token> = Vec::new();
        let mut is_stringize = false;
        let mut is_combine = false;
        for macro_tok in macro_body {
            // TODO: refine code
            if macro_tok.val == "#" {
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
                    expanded.push(self.stringize(&args[position]));
                    is_stringize = false;
                } else if is_combine {
                    let mut last = expanded.pop().unwrap();
                    for t in &args[position] {
                        last.val += t.val.as_str();
                    }
                    expanded.push(last);
                    is_combine = false;
                } else {
                    for t in &args[position] {
                        let mut a = t.clone();
                        a.hideset.insert(name.to_string());
                        expanded.push(a);
                    }
                }
            } else {
                let mut a = macro_tok.clone();
                a.hideset.insert(name.to_string());
                expanded.push(a);
            }
        }
        self.unget_all(expanded);
    }
    fn expand(&mut self, token: Option<Token>) -> Option<Token> {
        token.and_then(|tok| {
            let name = tok.val.to_string();

            if tok.hideset.contains(name.as_str()) || !self.macro_map.contains_key(name.as_str()) {
                Some(tok)
            } else {
                // if cur token is macro:
                match self.macro_map.get(name.as_str()).unwrap().clone() {
                    Macro::Object(ref body) => self.expand_obj_macro(name, body),
                    Macro::FuncLike(ref body) => self.expand_func_macro(name, body),
                }
                self.get()
            }
        })
    }

    pub fn get(&mut self) -> Option<Token> {
        let token = self.read_token();
        let tok = token.and_then(|tok| {
            match &tok.kind {
                &TokenKind::Symbol(Symbol::Hash) => {
                    self.read_cpp_directive();
                    self.get()
                }
                &TokenKind::String(ref s) => {
                    if let TokenKind::String(s2) = self.peek_e().kind {
                        // TODO: makes no sense...
                        self.get_e();
                        let mut new_tok = tok.clone();
                        let mut concat_str = s.clone();
                        concat_str.push_str(s2.as_str());
                        new_tok.kind = TokenKind::String(concat_str);
                        Some(new_tok)
                    } else {
                        Some(tok.clone())
                    }
                }
                _ => Some(tok.clone()),
            }
        });
        self.expand(tok)
    }

    pub fn get_e(&mut self) -> Token {
        let tok = self.get();
        if tok.is_none() {
            error::error_exit(*self.cur_line.back_mut().unwrap(),
                              "expected a token, but reach EOF");
        }
        tok.unwrap()
    }

    pub fn peek(&mut self) -> Option<Token> {
        let tok = self.get();
        if tok.is_some() {
            self.unget(tok.clone().unwrap());
        }
        tok
    }

    pub fn peek_e(&mut self) -> Token {
        let tok = self.peek();
        if tok.is_none() {
            error::error_exit(*self.cur_line.back_mut().unwrap(),
                              "expected a token, but reach EOF");
        }
        tok.unwrap()
    }

    // for c preprocessor

    fn read_cpp_directive(&mut self) {
        let t = self.do_read_token(); // cpp directive
        match t.ok_or("error").unwrap().val.as_str() {
            "include" => self.read_include(),
            "define" => self.read_define(),
            "undef" => self.read_undef(),
            "if" => self.read_if(),
            "ifdef" => self.read_ifdef(),
            "ifndef" => self.read_ifndef(),
            "elif" => self.read_elif(),
            "else" => self.read_else(),
            _ => {}
        }
    }

    fn try_include(&mut self, filename: &str) -> Option<String> {
        let header_paths = vec!["./include/",
                                "/include/",
                                "/usr/include/",
                                "/usr/include/linux/",
                                "/usr/include/x86_64-linux-gnu/",
                                "./include/",
                                ""];
        let mut real_filename = String::new();
        let mut found = false;
        for header_path in header_paths {
            real_filename = format!("{}{}", header_path, filename);
            if path::Path::new(real_filename.as_str()).exists() {
                found = true;
                break;
            }
        }
        if found { Some(real_filename) } else { None }
    }
    fn read_headerfile_name(&mut self) -> String {
        let mut name = "".to_string();
        if self.skip_symbol(Symbol::Lt) {
            while !self.peek_char_is('>') {
                name.push(self.peek_next());
            }
            self.peek_next(); // >
        } else {
            let tok = self.do_read_token()
                .or_else(|| {
                             error::error_exit(*self.cur_line.back_mut().unwrap(),
                                               "expcted macro args");
                         })
                .unwrap();
            if let TokenKind::String(s) = tok.kind {
                name = s;
            } else {
                error::error_exit(*self.cur_line.back_mut().unwrap(), "expected '<' or '\"'");
            }
        }
        name
    }
    fn read_include(&mut self) {
        // this will be a function
        let filename = self.read_headerfile_name();
        let real_filename = match self.try_include(filename.as_str()) {
            Some(f) => f,
            _ => {
                println!("error: {}: not found '{}'",
                         *self.cur_line.back_mut().unwrap(),
                         filename);
                process::exit(-1)
            }
        };
        println!("include filename: {}", real_filename);

        let mut include_file = OpenOptions::new()
            .read(true)
            .open(real_filename.to_string())
            .unwrap();
        let mut body = String::with_capacity(512);
        include_file
            .read_to_string(&mut body)
            .ok()
            .expect("not found file");
        self.filename.push_back(real_filename);
        unsafe {
            self.peek.push_back(body.as_mut_vec().clone());
        }
        self.peek_pos.push_back(0);
        self.cur_line.push_back(1);
    }

    fn read_define_obj_macro(&mut self, name: String) {
        println!("\tmacro: {}", name);

        let mut body: Vec<Token> = Vec::new();
        print!("\tmacro body: ");
        loop {
            let c = self.do_read_token().unwrap();
            if c.kind == TokenKind::Newline {
                break;
            }
            print!("{}{}", if c.space { " " } else { "" }, c.val);
            body.push(c);
        }
        println!();
        self.register_obj_macro(name, body);
    }
    fn read_define_func_macro(&mut self, name: String) {
        // read macro arguments
        let mut args: HashMap<String, usize> = HashMap::new();
        let mut count = 0usize;
        loop {
            let arg = self.get()
                .or_else(|| {
                             error::error_exit(*self.cur_line.back_mut().unwrap(),
                                               "expcted macro args");
                         })
                .unwrap()
                .val;
            args.insert(arg, count);
            if self.skip_symbol(Symbol::ClosingParen) {
                break;
            }
            self.expect_skip_symbol(Symbol::Comma);
            count += 1;
        }

        let mut body: Vec<Token> = Vec::new();
        loop {
            let tok = self.do_read_token().unwrap();
            if tok.kind == TokenKind::Newline {
                break;
            }

            // if tok is a parameter of funclike macro,
            //  the kind of tok will be changed to MacroParam
            //  and set macro_position
            let maybe_macro_name = tok.val.as_str();
            if args.contains_key(maybe_macro_name) {
                let mut macro_param = tok.clone();
                macro_param.kind = TokenKind::MacroParam;
                macro_param.macro_position = *args.get(maybe_macro_name).unwrap();
                body.push(macro_param);
            } else {
                body.push(tok.clone());
            }
        }
        self.register_funclike_macro(name, body);
    }
    fn read_define(&mut self) {
        let mcro = self.do_read_token().unwrap();
        assert_eq!(mcro.kind, TokenKind::Identifier);

        let t = self.do_read_token().unwrap();
        if !t.space && t.val.as_str() == "(" {
            self.read_define_func_macro(mcro.val);
        } else {
            self.unget(t);
            self.read_define_obj_macro(mcro.val);
        }
    }
    fn read_undef(&mut self) {
        let mcro = self.do_read_token().unwrap();
        assert_eq!(mcro.kind, TokenKind::Identifier);
        self.macro_map.remove(mcro.val.as_str());
    }

    fn register_obj_macro(&mut self, name: String, body: Vec<Token>) {
        self.macro_map.insert(name, Macro::Object(body));
    }
    fn register_funclike_macro(&mut self, name: String, body: Vec<Token>) {
        self.macro_map.insert(name, Macro::FuncLike(body));
    }

    fn read_defined_op(&mut self) -> Token {
        // TODO: add err handler
        let mut tok = self.do_read_token().unwrap();
        if tok.val == "(" {
            tok = self.do_read_token().unwrap();
            self.expect_skip_symbol(Symbol::ClosingParen);
        }
        if self.macro_map.contains_key(tok.val.as_str()) {
            Token::new(TokenKind::IntNumber(1),
                       "",
                       0,
                       0,
                       *self.cur_line.back_mut().unwrap())
        } else {
            Token::new(TokenKind::IntNumber(0),
                       "",
                       0,
                       0,
                       *self.cur_line.back_mut().unwrap())
        }
    }
    fn read_intexpr_line(&mut self) -> Vec<Token> {
        let mut v: Vec<Token> = Vec::new();
        loop {
            let mut tok = self.do_read_token()
                .or_else(|| {
                             error::error_exit(*self.cur_line.back_mut().unwrap(),
                                               "expect a token, but reach EOF")
                         })
                .unwrap();
            tok = self.expand(Some(tok)).unwrap();
            if tok.kind == TokenKind::Newline {
                break;
            } else {
                tok = self.convert_to_keyword_or_symbol(tok);
                if tok.val == "defined" {
                    v.push(self.read_defined_op());
                } else if tok.kind == TokenKind::Identifier {
                    // identifier in expr line is replaced with 0i
                    v.push(Token::new(TokenKind::IntNumber(0),
                                      "",
                                      0,
                                      0,
                                      *self.cur_line.back_mut().unwrap()));
                } else {
                    v.push(tok);
                }
            }
        }
        v
    }
    fn read_constexpr(&mut self) -> bool {
        let expr_line = self.read_intexpr_line();
        self.buf.push_back(VecDeque::new());

        self.unget(Token::new(TokenKind::Symbol(Symbol::Semicolon), "", 0, 0, 0));
        self.unget_all(expr_line);

        let node = parser::Parser::new(self).run_as_expr().ok().unwrap();

        self.buf.pop_back();

        node.eval_constexpr() != 0
    }

    fn do_read_if(&mut self, cond: bool) {
        self.cond_stack.push(cond);
        if !cond {
            self.skip_cond_include();
        }
    }
    fn read_if(&mut self) {
        let cond = self.read_constexpr();
        self.do_read_if(cond);
    }
    fn read_ifdef(&mut self) {
        let mcro_name = self.do_read_token()
            .or_else(|| error::error_exit(*self.cur_line.back_mut().unwrap(), "expected macro"))
            .unwrap()
            .val;
        let macro_is_defined = self.macro_map.contains_key(mcro_name.as_str());
        self.do_read_if(macro_is_defined);
    }
    fn read_ifndef(&mut self) {
        let mcro_name = self.do_read_token()
            .or_else(|| error::error_exit(*self.cur_line.back_mut().unwrap(), "expected macro"))
            .unwrap()
            .val;
        let macro_is_undefined = !self.macro_map.contains_key(mcro_name.as_str());
        self.do_read_if(macro_is_undefined);
    }
    fn read_elif(&mut self) {
        if self.cond_stack[self.cond_stack.len() - 1] || !self.read_constexpr() {
            self.skip_cond_include();
        } else {
            self.cond_stack.pop();
            self.cond_stack.push(true);
        }
    }
    fn read_else(&mut self) {
        if self.cond_stack[self.cond_stack.len() - 1] {
            self.skip_cond_include();
        }
    }

    fn skip_cond_include(&mut self) {
        let mut nest = 0;
        let get_tok = |lex: &mut Lexer| -> Token {
            lex.do_read_token()
                .or_else(|| error::error_exit(*lex.cur_line.back_mut().unwrap(), "reach EOF"))
                .unwrap()
        };
        loop {
            if self.peek_next() != '#' {
                continue;
            }

            let tok = get_tok(self);
            if nest == 0 {
                match tok.val.as_str() {
                    "else" | "elif" | "endif" => {
                        let line = *self.cur_line.back_mut().unwrap();
                        self.unget(tok);
                        self.unget(Token::new(TokenKind::Identifier, "#", 0, 0, line));
                        return;
                    }
                    _ => {}
                }
            }

            match tok.val.as_str() {
                "if" | "ifdef" | "ifndef" => nest += 1,
                "endif" => nest -= 1,
                _ => {}
            }
            // TODO: if nest < 0 then?
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
