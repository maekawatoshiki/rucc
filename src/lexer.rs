use std::fs::OpenOptions;
use std::io::prelude::*;
use std::str;
use std::collections::VecDeque;
use std::path;
use std::process;
use std::collections::{HashSet, HashMap};
use error;
use parser;
use parser::{ParseR, Error};

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
pub enum TokenKind {
    MacroParam,
    Keyword(Keyword),
    Identifier(String),
    IntNumber(i64),
    FloatNumber(f64),
    String(String),
    Char,
    Symbol(Symbol),
    Newline,
}

macro_rules! ident_val {
    ($e:expr) => {
        match &$e.kind {
            &TokenKind::Identifier(ref ident) => ident.to_string(),
            _ => "".to_string()
        }
    }
}
macro_rules! ident_mut_val {
    ($e:expr) => {
        match &mut $e.kind {
            &mut TokenKind::Identifier(ref mut ident) => ident,
            _ => panic!()
        }
    }
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
    pub fn get_filename(&mut self) -> String {
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
        let nextc = self.peek.back().unwrap()[*self.peek_pos.back_mut().unwrap() + 1] as char;
        nextc == ch
    }
    fn peek_char_is(&mut self, ch: char) -> bool {
        let line = *self.cur_line.back().unwrap();
        let errf =
            || -> Option<char> { error::error_exit(line, format!("expected '{}'", ch).as_str()); };
        let peekc = self.peek_get().or_else(errf).unwrap();
        peekc == ch
    }

    pub fn peek_keyword_token_is(&mut self, expect: Keyword) -> bool {
        let peek = self.peek_e();
        peek.kind == TokenKind::Keyword(expect)
    }
    pub fn peek_symbol_token_is(&mut self, expect: Symbol) -> bool {
        let peek = self.peek_e();
        peek.kind == TokenKind::Symbol(expect)
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
    pub fn skip_keyword(&mut self, keyword: Keyword) -> ParseR<bool> {
        let tok = try!(self.get());
        Ok(if tok.kind == TokenKind::Keyword(keyword) {
               true
           } else {
               self.buf.back_mut().unwrap().push_back(tok);
               false
           })
    }
    pub fn skip_symbol(&mut self, sym: Symbol) -> ParseR<bool> {
        let tok = try!(self.get());
        Ok(if tok.kind == TokenKind::Symbol(sym) {
               true
           } else {
               self.buf.back_mut().unwrap().push_back(tok);
               false
           })
    }
    pub fn expect_skip_keyword(&mut self, expect: Keyword) -> ParseR<bool> {
        self.skip_keyword(expect.clone())
    }
    pub fn expect_skip_symbol(&mut self, expect: Symbol) -> ParseR<bool> {
        self.skip_symbol(expect.clone())
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
        Token::new(TokenKind::Identifier(ident),
                   "",
                   0,
                   pos,
                   *self.cur_line.back_mut().unwrap())
    }
    fn read_number_literal(&mut self) -> Token {
        let mut num = String::with_capacity(8);
        let mut is_float = false;
        let mut last = self.peek_get().unwrap();
        let pos = *self.peek_pos.back().unwrap();
        loop {
            match self.peek_get() {
                Some(c) => {
                    num.push(c);
                    is_float = is_float || c == '.';
                    let is_f = "eEpP".contains(last) && "+-".contains(c);
                    if !c.is_alphanumeric() && c != '.' && !is_f {
                        is_float = is_float || is_f;
                        num.pop();
                        break;
                    }
                    last = c;
                }
                _ => break,
            };
            self.peek_next();
        }
        if is_float {
            // TODO: this is to delete suffix like 'F', but not efficient
            loop {
                if let Some(last) = num.chars().last() {
                    if last.is_alphabetic() {
                        num.pop();
                    } else {
                        break;
                    }
                }
            }

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
        let mut n = 0u64;
        for c in num_literal.chars() {
            match c {
                '0'...'9' => n = n * 10 + c.to_digit(10).unwrap() as u64,
                _ => {} // TODO: suffix
            }
        }
        n as i64
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
        Token::new(TokenKind::Identifier(sym),
                   "",
                   0,
                   pos,
                   *self.cur_line.back_mut().unwrap())
    }
    fn read_escaped_char(&mut self) -> char {
        let c = self.peek_next();
        match c {
            '\'' | '"' | '?' | '\\' => c,
            '0' => '\x00',
            'a' => '\x07',
            'b' => '\x08',
            'f' => '\x0c',
            'n' => '\x0a',
            'r' => '\x0d',
            't' => '\x09',
            'v' => '\x0b',
            'x' => {
                let mut hex = "".to_string();
                loop {
                    let c = self.peek_get().unwrap();
                    match c {
                        '0'...'9' | 'a'...'f' | 'A'...'F' => hex.push(c),
                        _ => break,
                    }
                    self.peek_next();
                }
                self.read_hex_num(hex.as_str()) as i32 as u8 as char
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

    pub fn do_read_token(&mut self) -> ParseR<Token> {
        if !self.buf.back_mut().unwrap().is_empty() {
            return match self.buf.back_mut().unwrap().pop_back() {
                       Some(tok) => Ok(tok),
                       None => Err(Error::EOF),
                   };
        }

        match self.peek_get() {
            Some(c) => {
                match c {
                    'a'...'z' | 'A'...'Z' | '_' => Ok(self.read_identifier()),
                    ' ' | '\t' => {
                        self.peek_next();
                        self.do_read_token()
                            // set a leading space
                            .and_then(|tok| {
                                let mut t = tok;
                                t.space = true;
                                Ok(t)
                            })
                    }
                    '0'...'9' => Ok(self.read_number_literal()),
                    '\"' => Ok(self.read_string_literal()),
                    '\'' => Ok(self.read_char_literal()),
                    '\n' => Ok(self.read_newline()),
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
                            Ok(self.read_symbol())
                        }
                    }
                    _ => Ok(self.read_symbol()),
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
                    Err(Error::EOF)
                }
            }
        }
    }
    pub fn read_token(&mut self) -> ParseR<Token> {
        let token = self.do_read_token();
        token.and_then(|tok| match tok.kind {
                           TokenKind::Newline => self.read_token(),
                           TokenKind::Identifier(_) => Ok(self.convert_to_keyword_or_symbol(tok)),
                           _ => Ok(tok),
                       })
    }
    fn convert_to_keyword_or_symbol(&mut self, token: Token) -> Token {
        let pos = token.pos;
        let line = token.line;
        let val = ident_val!(token);

        if val == "sizeof" {
            return Token::new(TokenKind::Symbol(Symbol::Sizeof), "", 0, pos, line);
        }

        if val.len() > 0 && val.chars().nth(0).unwrap().is_alphanumeric() {
            let keyw = match val.as_str() {
                "typedef" => TokenKind::Keyword(Keyword::Typedef),
                "extern" => TokenKind::Keyword(Keyword::Extern),
                "auto" => TokenKind::Keyword(Keyword::Auto),
                "register" => TokenKind::Keyword(Keyword::Register),
                "static" => TokenKind::Keyword(Keyword::Static),
                "restrict" => TokenKind::Keyword(Keyword::Restrict),
                "const" => TokenKind::Keyword(Keyword::Const),
                "volatile" => TokenKind::Keyword(Keyword::Volatile),
                "void" => TokenKind::Keyword(Keyword::Void),
                "signed" => TokenKind::Keyword(Keyword::Signed),
                "unsigned" => TokenKind::Keyword(Keyword::Unsigned),
                "char" => TokenKind::Keyword(Keyword::Char),
                "int" => TokenKind::Keyword(Keyword::Int),
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
                "break" => TokenKind::Keyword(Keyword::Break),
                "continue" => TokenKind::Keyword(Keyword::Continue),
                "return" => TokenKind::Keyword(Keyword::Return),
                _ => return token,
            };
            return Token::new(keyw, "", 0, pos, line);
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

        Token::new(symbol, "", 0, pos, line)
    }

    fn expand_obj_macro(&mut self, name: String, macro_body: &Vec<Token>) -> ParseR<()> {
        let mut body = Vec::new();
        for tok in macro_body {
            body.push({
                          let mut t = (*tok).clone();
                          t.hideset.insert(name.to_string());
                          t
                      });
        }
        self.unget_all(body);
        Ok(())
    }
    fn read_one_arg(&mut self, end: &mut bool) -> ParseR<Vec<Token>> {
        let mut n = 0;
        let mut arg = Vec::new();
        loop {
            let tok = try!(self.do_read_token());
            let val = ident_val!(tok);
            if n == 0 {
                if val == ")" {
                    *end = true;
                    // self.unget(tok);
                    break;
                } else if val == "," {
                    break;
                }
            }
            match val.as_str() {
                "(" => n += 1,
                ")" => n -= 1,
                _ => {}
            }
            arg.push(tok);
        }
        Ok(arg)
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
                                  TokenKind::Identifier(ref i) => format!("{}", i),
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
    fn expand_func_macro(&mut self, name: String, macro_body: &Vec<Token>) -> ParseR<()> {
        // expect '(', (self.skip can't be used because skip uses 'self.get' that uses MACRO_MAP using Mutex
        let expect_bracket = try!(self.read_token());
        if expect_bracket.kind != TokenKind::Symbol(Symbol::OpeningParen) {
            error::error_exit(*self.cur_line.back_mut().unwrap(), "expected '('");
        }

        let mut args = Vec::new();
        let mut end = false;
        while !end {
            args.push(try!(self.read_one_arg(&mut end)));
        }

        let mut expanded: Vec<Token> = Vec::new();
        let mut is_stringize = false;
        let mut is_combine = false;
        for macro_tok in macro_body {
            // TODO: refine code
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
                    let stringized = self.stringize(&args[position]);
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
                    let l = self.buf.back().unwrap().len();
                    self.unget_all(args[position].clone());
                    while self.buf.back().unwrap().len() > l {
                        expanded.push(try!(self.get()));
                    }
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
            tok.hideset.insert(name.to_string());
        }

        self.unget_all(expanded);
        Ok(())
    }
    fn expand(&mut self, token: ParseR<Token>) -> ParseR<Token> {
        token.and_then(|tok| {
            let name = ident_val!(tok);
            if tok.hideset.contains(name.as_str()) || !self.macro_map.contains_key(name.as_str()) {
                Ok(tok)
            } else {
                // if cur token is macro:
                try!(match self.macro_map.get(name.as_str()).unwrap().clone() {
                         Macro::Object(ref body) => self.expand_obj_macro(name, body),
                         Macro::FuncLike(ref body) => self.expand_func_macro(name, body),
                     });
                self.get()
            }
        })
    }

    pub fn get(&mut self) -> ParseR<Token> {
        let tok = self.read_token()
            .and_then(|tok| {
                match &tok.kind {
                    &TokenKind::Symbol(Symbol::Hash) => {
                        try!(self.read_cpp_directive());
                        self.get()
                    }
                    &TokenKind::String(ref s) => {
                        if let TokenKind::String(s2) = try!(self.peek()).kind {
                            // TODO: makes no sense...
                            try!(self.get());
                            let mut new_tok = tok.clone();
                            let mut concat_str = s.clone();
                            concat_str.push_str(s2.as_str());
                            new_tok.kind = TokenKind::String(concat_str);
                            Ok(new_tok)
                        } else {
                            Ok(tok.clone())
                        }
                    }
                    _ => Ok(tok.clone()),
                }
            });
        self.expand(tok)
    }

    pub fn get_e(&mut self) -> Token {
        let tok = self.get();
        match tok {
            Ok(ok) => ok,
            Err(_) => {
                error::error_exit(*self.cur_line.back_mut().unwrap(),
                                  "expected a token, but reach EOF");
            }
        }
    }

    pub fn peek(&mut self) -> ParseR<Token> {
        self.get()
            .and_then(|tok| {
                          self.unget(tok.clone());
                          Ok(tok)
                      })
    }

    pub fn peek_e(&mut self) -> Token {
        let tok = self.peek();
        match tok {
            Ok(ok) => ok,
            Err(_) => {
                error::error_exit(*self.cur_line.back_mut().unwrap(),
                                  "expected a token, but reach EOF");
            }
        }
    }

    // for c preprocessor

    fn read_cpp_directive(&mut self) -> ParseR<()> {
        let tok = self.do_read_token(); // cpp directive
        match tok { 
            Ok(t) => {
                match ident_val!(t).as_str() {
                    "include" => self.read_include(),
                    "define" => self.read_define(),
                    "undef" => self.read_undef(),
                    "if" => self.read_if(),
                    "ifdef" => self.read_ifdef(),
                    "ifndef" => self.read_ifndef(),
                    "elif" => self.read_elif(),
                    "else" => self.read_else(),
                    _ => Ok(()),
                }
            }
            Err(e) => Err(e),
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
    fn read_headerfile_name(&mut self) -> ParseR<String> {
        let mut name = "".to_string();
        if try!(self.skip_symbol(Symbol::Lt)) {
            while !self.peek_char_is('>') {
                name.push(self.peek_next());
            }
            self.peek_next(); // >
        } else {
            let tok = try!(self.do_read_token());
            if let TokenKind::String(s) = tok.kind {
                name = s;
            } else {
                error::error_exit(*self.cur_line.back_mut().unwrap(), "expected '<' or '\"'");
            }
        }
        Ok(name)
    }
    fn read_include(&mut self) -> ParseR<()> {
        // this will be a function
        let filename = try!(self.read_headerfile_name());
        let real_filename = match self.try_include(filename.as_str()) {
            Some(f) => f,
            _ => {
                println!("error: {}: not found '{}'",
                         *self.cur_line.back_mut().unwrap(),
                         filename);
                process::exit(-1)
            }
        };
        // DEBUG: println!("include filename: {}", real_filename);

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
        Ok(())
    }

    fn read_define_obj_macro(&mut self, name: String) -> ParseR<()> {
        // DEBUG: println!("\tmacro: {}", name);

        let mut body: Vec<Token> = Vec::new();
        // DEBUG: print!("\tmacro body: ");
        loop {
            let c = try!(self.do_read_token());
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
        let mut args: HashMap<String, usize> = HashMap::new();
        let mut count = 0usize;
        loop {
            let arg = ident_val!(try!(self.get()));
            args.insert(arg, count);
            if try!(self.skip_symbol(Symbol::ClosingParen)) {
                break;
            }
            try!(self.expect_skip_symbol(Symbol::Comma));
            count += 1;
        }

        let mut body = Vec::new();
        // print!("\tmacro body: ");
        loop {
            let tok = try!(self.do_read_token());
            if tok.kind == TokenKind::Newline {
                break;
            }

            // if tok is a parameter of funclike macro,
            //  the kind of tok will be changed to MacroParam
            //  and set macro_position
            let maybe_macro_name = ident_val!(tok);
            // print!("{}{}", if tok.space { " " } else { "" }, tok.val);
            if args.contains_key(maybe_macro_name.as_str()) {
                let mut macro_param = tok.clone();
                macro_param.kind = TokenKind::MacroParam;
                macro_param.macro_position = *args.get(maybe_macro_name.as_str()).unwrap();
                body.push(macro_param);
            } else {
                body.push(tok.clone());
            }
        }
        self.register_funclike_macro(name, body);
        Ok(())
    }
    fn read_define(&mut self) -> ParseR<()> {
        let mcro = try!(self.do_read_token());
        // assert_eq!(mcro.kind, TokenKind::Identifier);
        // println!("define: {}", mcro.val);

        let t = try!(self.do_read_token());
        if !t.space && ident_val!(t).as_str() == "(" {
            self.read_define_func_macro(ident_val!(mcro))
        } else {
            self.unget(t);
            self.read_define_obj_macro(ident_val!(mcro))
        }
    }
    fn read_undef(&mut self) -> ParseR<()> {
        let mcro = try!(self.do_read_token());
        // assert_eq!(mcro.kind, TokenKind::Identifier);
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
        // TODO: add err handler
        let mut tok = try!(self.do_read_token());
        if ident_val!(tok) == "(" {
            tok = try!(self.do_read_token());
            try!(self.expect_skip_symbol(Symbol::ClosingParen));
        }
        Ok(if self.macro_map.contains_key(ident_val!(tok).as_str()) {
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
           })
    }
    fn read_intexpr_line(&mut self) -> ParseR<Vec<Token>> {
        let mut v = Vec::new();
        loop {
            let mut tok = try!(self.do_read_token());
            tok = try!(self.expand(Ok(tok)));
            if tok.kind == TokenKind::Newline {
                break;
            } else {
                tok = self.convert_to_keyword_or_symbol(tok);
                match tok.kind {
                    TokenKind::Identifier(s) => {
                        if s == "defined" {
                            v.push(try!(self.read_defined_op()));
                        } else {
                            // identifier in expr line is replaced with 0i
                            v.push(Token::new(TokenKind::IntNumber(0),
                                              "",
                                              0,
                                              0,
                                              *self.cur_line.back_mut().unwrap()));
                        }
                    }
                    _ => v.push(tok),
                }
            }
        }
        Ok(v)
    }
    fn read_constexpr(&mut self) -> ParseR<bool> {
        let expr_line = try!(self.read_intexpr_line());
        self.buf.push_back(VecDeque::new());

        self.unget(Token::new(TokenKind::Symbol(Symbol::Semicolon), "", 0, 0, 0));
        self.unget_all(expr_line);

        let node = parser::Parser::new(self).run_as_expr().ok().unwrap();

        self.buf.pop_back();

        Ok(node.eval_constexpr() != 0)
    }

    fn do_read_if(&mut self, cond: bool) -> ParseR<()> {
        self.cond_stack.push(cond);
        if !cond {
            try!(self.skip_cond_include());
        }
        Ok(())
    }
    fn read_if(&mut self) -> ParseR<()> {
        let cond = try!(self.read_constexpr());
        self.do_read_if(cond)
    }
    fn read_ifdef(&mut self) -> ParseR<()> {
        let mcro_name = ident_val!(try!(self.do_read_token()));
        let macro_is_defined = self.macro_map.contains_key(mcro_name.as_str());
        self.do_read_if(macro_is_defined)
    }
    fn read_ifndef(&mut self) -> ParseR<()> {
        let mcro_name = ident_val!(try!(self.do_read_token()));
        let macro_is_undefined = !self.macro_map.contains_key(mcro_name.as_str());
        self.do_read_if(macro_is_undefined)
    }
    fn read_elif(&mut self) -> ParseR<()> {
        if self.cond_stack[self.cond_stack.len() - 1] || !try!(self.read_constexpr()) {
            try!(self.skip_cond_include());
        } else {
            self.cond_stack.pop();
            self.cond_stack.push(true);
        }
        Ok(())
    }
    fn read_else(&mut self) -> ParseR<()> {
        if self.cond_stack[self.cond_stack.len() - 1] {
            try!(self.skip_cond_include());
        }
        Ok(())
    }

    fn skip_cond_include(&mut self) -> ParseR<()> {
        let mut nest = 0;
        loop {
            if self.peek_next() != '#' {
                continue;
            }

            let tok = try!(self.do_read_token());
            let val = ident_val!(tok);
            if nest == 0 {
                match val.as_str() {
                    "else" | "elif" | "endif" => {
                        let line = *self.cur_line.back_mut().unwrap();
                        self.unget(tok);
                        self.unget(Token::new(TokenKind::Identifier("#".to_string()),
                                              "",
                                              0,
                                              0,
                                              line));
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
