use std::fs::OpenOptions;
use std::io::prelude::*;
use std::str;
use std::collections::{VecDeque, LinkedList};
use std::path;
use std::process;
use std::collections::{HashSet, HashMap};
use error;
use parser;
use MACRO_MAP;

#[derive(Debug)]
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
    IntNumber,
    FloatNumber,
    String,
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
    pub line: i32,
}

impl Token {
    pub fn new(kind: TokenKind, val: &str, macro_position: usize, line: i32) -> Token {
        Token {
            kind: kind,
            space: false,
            val: val.to_string(),
            macro_position: macro_position,
            hideset: HashSet::new(),
            line: line,
        }
    }
}

#[derive(Clone)]
pub struct Lexer {
    pub cur_line: i32,
    filename: LinkedList<String>,
    peek: VecDeque<Vec<u8>>,
    peek_pos: VecDeque<usize>,
    buf: VecDeque<VecDeque<Token>>,
    cond_stack: Vec<bool>,
}

impl Lexer {
    pub fn new(filename: String) -> Lexer {
        let mut buf: VecDeque<VecDeque<Token>> = VecDeque::new();
        buf.push_back(VecDeque::new());

        let mut file = OpenOptions::new()
            .read(true)
            .open(filename.to_string())
            .unwrap();
        let mut file_body = String::new();
        file.read_to_string(&mut file_body);
        let mut peek = VecDeque::new();
        unsafe {
            peek.push_back(file_body.as_mut_vec().clone());
        }

        let mut peek_pos = VecDeque::new();
        peek_pos.push_back(0);

        let mut filenames = LinkedList::new();
        filenames.push_back(filename);

        Lexer {
            cur_line: 1,
            filename: filenames,
            peek: peek,
            peek_pos: peek_pos,
            buf: buf,
            cond_stack: Vec::new(),
        }
    }
    pub fn get_filename(self) -> String {
        self.filename.back().unwrap().clone()
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
        *peek_pos += 1;
        self.peek.back_mut().unwrap()[*peek_pos - 1] as char
    }
    fn peek_next_char_is(&mut self, ch: char) -> bool {
        self.peek_next();
        let nextc = self.peek_next();
        *self.peek_pos.back_mut().unwrap() -= 2;
        nextc == ch
    }
    fn peek_char_is(&mut self, ch: char) -> bool {
        let line = self.cur_line;
        let errf =
            || -> Option<char> { error::error_exit(line, format!("expected '{}'", ch).as_str()); };

        let peekc = self.peek_get().or_else(errf).unwrap();
        peekc == ch
    }

    pub fn peek_token_is(&mut self, expect: &str) -> bool {
        let peek = self.peek_e();
        peek.val == expect && peek.kind != TokenKind::String && peek.kind != TokenKind::Char
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
        let n = next.clone();
        self.unget(next);
        self.unget(peek);
        n.val == expect && n.kind != TokenKind::String && n.kind != TokenKind::Char
    }
    pub fn next_keyword_token_is(&mut self, expect: Keyword) -> bool {
        let peek = self.get_e();
        let next = self.get_e();
        let n = next.clone();
        self.unget(next);
        self.unget(peek);
        n.kind == TokenKind::Keyword(expect)
    }
    pub fn next_symbol_token_is(&mut self, expect: Symbol) -> bool {
        let peek = self.get_e();
        let next = self.get_e();
        let n = next.clone();
        self.unget(next);
        self.unget(peek);
        n.kind == TokenKind::Symbol(expect)
    }
    pub fn skip(&mut self, s: &str) -> bool {
        let next = self.get();
        match next {
            Some(n) => {
                if n.val == s && n.kind != TokenKind::String && n.kind != TokenKind::Char {
                    true
                } else {
                    self.buf.back_mut().unwrap().push_back(n);
                    false
                }
            }
            None => {
                error::error_exit(self.cur_line,
                                  format!("expect '{}' but reach EOF", s).as_str())
            }
        }
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
            None => {
                error::error_exit(self.cur_line,
                                  format!("expect '{:?}' but reach EOF", keyword).as_str())
            }
        }
    }
    pub fn skip_symbol(&mut self, sym: Symbol) -> bool {
        match self.get() {
            Some(n) => {
                if n.kind == TokenKind::Symbol(sym) {
                    true
                } else {
                    self.buf.back_mut().unwrap().push_back(n);
                    false
                }
            }
            None => {
                error::error_exit(self.cur_line,
                                  format!("expect '{:?}' but reach EOF", sym).as_str())
            }
        }
    }
    pub fn expect_skip(&mut self, expect: &str) -> bool {
        if !self.skip(expect) {
            error::error_exit(self.cur_line,
                              format!("expected the keyword '{}'", expect).as_str());
        }
        true
    }
    pub fn expect_skip_keyword(&mut self, expect: Keyword) -> bool {
        if !self.skip_keyword(expect.clone()) {
            error::error_exit(self.cur_line,
                              format!("expected the keyword '{:?}'", expect).as_str());
        }
        true
    }
    pub fn expect_skip_symbol(&mut self, expect: Symbol) -> bool {
        if !self.skip_symbol(expect.clone()) {
            error::error_exit(self.cur_line,
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
        let mut ident = String::new();
        loop {
            match self.peek_get() {
                Some(c) => {
                    match c {
                        'a'...'z' | 'A'...'Z' | '_' | '0'...'9' => ident.push(c),
                        _ => break,
                    }
                }
                _ => break,
            };
            self.peek_next();
        }
        Token::new(TokenKind::Identifier, ident.as_str(), 0, self.cur_line)
    }
    fn read_number_literal(&mut self) -> Token {
        let mut num = String::new();
        let mut is_float = false;
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
            Token::new(TokenKind::FloatNumber, num.as_str(), 0, self.cur_line)
        } else {
            Token::new(TokenKind::IntNumber, num.as_str(), 0, self.cur_line)
        }
    }
    pub fn read_newline(&mut self) -> Token {
        self.peek_next();
        self.cur_line += 1;
        Token::new(TokenKind::Newline, "", 0, self.cur_line)
    }
    pub fn read_symbol(&mut self) -> Token {
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
        Token::new(TokenKind::Identifier, sym.as_str(), 0, self.cur_line)
    }
    fn read_string_literal(&mut self) -> Token {
        self.peek_next();
        let mut s = String::new();
        while !self.peek_char_is('\"') {
            s.push(self.peek_next());
        }
        self.peek_next();
        Token::new(TokenKind::String, s.as_str(), 0, self.cur_line)
    }
    fn read_char_literal(&mut self) -> Token {
        self.peek_next();
        let mut s = String::new();
        while !self.peek_char_is('\'') {
            s.push(self.peek_next());
        }
        self.peek_next();
        Token::new(TokenKind::Char, s.as_str(), 0, self.cur_line)
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
        match token.val.as_str() {
            "typedef" => Token::new(TokenKind::Keyword(Keyword::Typedef), "", 0, self.cur_line),
            "extern" => Token::new(TokenKind::Keyword(Keyword::Extern), "", 0, self.cur_line),
            "auto" => Token::new(TokenKind::Keyword(Keyword::Auto), "", 0, self.cur_line),
            "register" => Token::new(TokenKind::Keyword(Keyword::Register), "", 0, self.cur_line),
            "static" => Token::new(TokenKind::Keyword(Keyword::Static), "", 0, self.cur_line),
            "restrict" => Token::new(TokenKind::Keyword(Keyword::Restrict), "", 0, self.cur_line),
            "const" => Token::new(TokenKind::Keyword(Keyword::Const), "", 0, self.cur_line),
            "volatile" => Token::new(TokenKind::Keyword(Keyword::Volatile), "", 0, self.cur_line),
            "void" => Token::new(TokenKind::Keyword(Keyword::Void), "", 0, self.cur_line),
            "signed" => Token::new(TokenKind::Keyword(Keyword::Signed), "", 0, self.cur_line),
            "unsigned" => Token::new(TokenKind::Keyword(Keyword::Unsigned), "", 0, self.cur_line),
            "char" => Token::new(TokenKind::Keyword(Keyword::Char), "", 0, self.cur_line),
            "int" => Token::new(TokenKind::Keyword(Keyword::Int), "", 0, self.cur_line),
            "short" => Token::new(TokenKind::Keyword(Keyword::Short), "", 0, self.cur_line),
            "long" => Token::new(TokenKind::Keyword(Keyword::Long), "", 0, self.cur_line),
            "float" => Token::new(TokenKind::Keyword(Keyword::Float), "", 0, self.cur_line),
            "double" => Token::new(TokenKind::Keyword(Keyword::Double), "", 0, self.cur_line),
            "struct" => Token::new(TokenKind::Keyword(Keyword::Struct), "", 0, self.cur_line),
            "union" => Token::new(TokenKind::Keyword(Keyword::Union), "", 0, self.cur_line),
            "enum" => Token::new(TokenKind::Keyword(Keyword::Enum), "", 0, self.cur_line),
            "inline" => Token::new(TokenKind::Keyword(Keyword::Inline), "", 0, self.cur_line),
            "noreturn" => Token::new(TokenKind::Keyword(Keyword::Noreturn), "", 0, self.cur_line),
            "if" => Token::new(TokenKind::Keyword(Keyword::If), "", 0, self.cur_line),
            "else" => Token::new(TokenKind::Keyword(Keyword::Else), "", 0, self.cur_line),
            "for" => Token::new(TokenKind::Keyword(Keyword::For), "", 0, self.cur_line),
            "while" => Token::new(TokenKind::Keyword(Keyword::While), "", 0, self.cur_line),
            "return" => Token::new(TokenKind::Keyword(Keyword::Return), "", 0, self.cur_line),
            "++" => Token::new(TokenKind::Symbol(Symbol::Inc), "", 0, self.cur_line), 
            "--" => Token::new(TokenKind::Symbol(Symbol::Dec), "", 0, self.cur_line),
            "(" => {
                Token::new(TokenKind::Symbol(Symbol::OpeningParen),
                           "",
                           0,
                           self.cur_line)
            }
            ")" => {
                Token::new(TokenKind::Symbol(Symbol::ClosingParen),
                           "",
                           0,
                           self.cur_line)
            }
            "[" => {
                Token::new(TokenKind::Symbol(Symbol::OpeningBoxBracket),
                           "",
                           0,
                           self.cur_line)
            }
            "]" => {
                Token::new(TokenKind::Symbol(Symbol::ClosingBoxBracket),
                           "",
                           0,
                           self.cur_line)
            }
            "{" => {
                Token::new(TokenKind::Symbol(Symbol::OpeningBrace),
                           "",
                           0,
                           self.cur_line)
            }
            "}" => {
                Token::new(TokenKind::Symbol(Symbol::ClosingBrace),
                           "",
                           0,
                           self.cur_line)
            }
            "." => Token::new(TokenKind::Symbol(Symbol::Point), "", 0, self.cur_line),
            "," => Token::new(TokenKind::Symbol(Symbol::Comma), "", 0, self.cur_line),
            ";" => Token::new(TokenKind::Symbol(Symbol::Semicolon), "", 0, self.cur_line),
            ":" => Token::new(TokenKind::Symbol(Symbol::Colon), "", 0, self.cur_line),
            "->" => Token::new(TokenKind::Symbol(Symbol::Arrow), "", 0, self.cur_line),
            "+" => Token::new(TokenKind::Symbol(Symbol::Add), "", 0, self.cur_line),
            "-" => Token::new(TokenKind::Symbol(Symbol::Sub), "", 0, self.cur_line),
            "!" => Token::new(TokenKind::Symbol(Symbol::Not), "", 0, self.cur_line),
            "~" => Token::new(TokenKind::Symbol(Symbol::BitwiseNot), "", 0, self.cur_line),
            "*" => Token::new(TokenKind::Symbol(Symbol::Asterisk), "", 0, self.cur_line),
            "&" => Token::new(TokenKind::Symbol(Symbol::Ampersand), "", 0, self.cur_line),
            "/" => Token::new(TokenKind::Symbol(Symbol::Div), "", 0, self.cur_line),
            "%" => Token::new(TokenKind::Symbol(Symbol::Mod), "", 0, self.cur_line),
            "<<" => Token::new(TokenKind::Symbol(Symbol::Shl), "", 0, self.cur_line),
            ">>" => Token::new(TokenKind::Symbol(Symbol::Shr), "", 0, self.cur_line),
            "<" => Token::new(TokenKind::Symbol(Symbol::Lt), "", 0, self.cur_line),
            "<=" => Token::new(TokenKind::Symbol(Symbol::Le), "", 0, self.cur_line),
            ">" => Token::new(TokenKind::Symbol(Symbol::Gt), "", 0, self.cur_line),
            ">=" => Token::new(TokenKind::Symbol(Symbol::Ge), "", 0, self.cur_line),
            "==" => Token::new(TokenKind::Symbol(Symbol::Eq), "", 0, self.cur_line),
            "!=" => Token::new(TokenKind::Symbol(Symbol::Ne), "", 0, self.cur_line),
            "^" => Token::new(TokenKind::Symbol(Symbol::Xor), "", 0, self.cur_line),
            "|" => Token::new(TokenKind::Symbol(Symbol::Or), "", 0, self.cur_line),
            "&&" => Token::new(TokenKind::Symbol(Symbol::LAnd), "", 0, self.cur_line),
            "||" => Token::new(TokenKind::Symbol(Symbol::LOr), "", 0, self.cur_line),
            "?" => Token::new(TokenKind::Symbol(Symbol::Question), "", 0, self.cur_line),
            "=" => Token::new(TokenKind::Symbol(Symbol::Assign), "", 0, self.cur_line),
            "+=" => Token::new(TokenKind::Symbol(Symbol::AssignAdd), "", 0, self.cur_line),
            "-=" => Token::new(TokenKind::Symbol(Symbol::AssignSub), "", 0, self.cur_line),
            "*=" => Token::new(TokenKind::Symbol(Symbol::AssignMul), "", 0, self.cur_line),
            "/=" => Token::new(TokenKind::Symbol(Symbol::AssignDiv), "", 0, self.cur_line),
            "%=" => Token::new(TokenKind::Symbol(Symbol::AssignMod), "", 0, self.cur_line),
            "<<=" => Token::new(TokenKind::Symbol(Symbol::AssignShl), "", 0, self.cur_line),
            ">>=" => Token::new(TokenKind::Symbol(Symbol::AssignShr), "", 0, self.cur_line),
            "&=" => Token::new(TokenKind::Symbol(Symbol::AssignAnd), "", 0, self.cur_line),
            "^=" => Token::new(TokenKind::Symbol(Symbol::AssignXor), "", 0, self.cur_line),
            "|=" => Token::new(TokenKind::Symbol(Symbol::AssignOr), "", 0, self.cur_line),
            "#" => Token::new(TokenKind::Symbol(Symbol::Hash), "", 0, self.cur_line),
            "..." => Token::new(TokenKind::Symbol(Symbol::Vararg), "", 0, self.cur_line),
            "sizeof" => Token::new(TokenKind::Symbol(Symbol::Sizeof), "", 0, self.cur_line),
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
                .or_else(|| error::error_exit(self.cur_line, "expected macro args but reach EOF"))
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
            string += format!("{}{}", (if token.space { " " } else { "" }), token.val).as_str();
        }
        Token::new(TokenKind::String, string.as_str(), 0, self.cur_line)
    }
    fn expand_func_macro(&mut self, name: String, macro_body: &Vec<Token>) {
        // expect '(', (self.skip can't be used because skip uses 'self.get' that uses MACRO_MAP using Mutex
        let expect_bracket = self.read_token()
            .or_else(|| error::error_exit(self.cur_line, "expected '(' but reach EOF"))
            .unwrap();
        if expect_bracket.kind != TokenKind::Symbol(Symbol::OpeningParen) {
            error::error_exit(self.cur_line, "expected '('");
        }

        let mut args: Vec<Vec<Token>> = Vec::new();
        // read macro arguments
        loop {
            let maybe_bracket = self.do_read_token()
                .or_else(|| error::error_exit(self.cur_line, "expected ')' but reach EOF"))
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
                // means ##
                if is_stringize {
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

            if tok.hideset.contains(name.as_str()) ||
               !MACRO_MAP.lock().unwrap().contains_key(name.as_str()) {
                Some(tok)
            } else {
                // if cur token is macro:
                match MACRO_MAP.lock().unwrap().get(name.as_str()).unwrap() {
                    &Macro::Object(ref body) => self.expand_obj_macro(name, body),
                    &Macro::FuncLike(ref body) => self.expand_func_macro(name, body),
                }
                self.get()
            }
        })
    }

    pub fn get(&mut self) -> Option<Token> {
        let t = self.read_token();
        let tok = match t {
            Some(tok) => {
                if tok.kind == TokenKind::Symbol(Symbol::Hash) {
                    self.read_cpp_directive();
                    self.get()
                } else if tok.kind == TokenKind::String && self.peek_e().kind == TokenKind::String {
                    let s = self.get_e().val;
                    let mut nt = tok.clone();
                    nt.val.push_str(s.as_str());
                    Some(nt)
                } else {
                    Some(tok)
                }
            }
            _ => return t,
        };
        self.expand(tok)
    }

    pub fn get_e(&mut self) -> Token {
        let tok = self.get();
        if tok.is_none() {
            error::error_exit(self.cur_line, "expected a token, but reach EOF");
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
            error::error_exit(self.cur_line, "expected a token, but reach EOF");
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
        } else if self.skip("\"") {
            while !self.peek_char_is('"') {
                name.push(self.peek_next());
            }
            self.peek_next(); // "
        } else {
            error::error_exit(self.cur_line, "expected '<' or '\"'");
        }
        name
    }
    fn read_include(&mut self) {
        // this will be a function
        let filename = self.read_headerfile_name();
        let real_filename = match self.try_include(filename.as_str()) {
            Some(f) => f,
            _ => {
                println!("error: {}: not found '{}'", self.cur_line, filename);
                process::exit(-1)
            }
        };
        println!("include filename: {}", real_filename);
        let mut file = OpenOptions::new()
            .read(true)
            .open(real_filename.to_string())
            .unwrap();
        let mut body = String::new();
        file.read_to_string(&mut body);
        self.filename.push_back(real_filename);
        unsafe {
            self.peek.push_back(body.as_mut_vec().clone());
        }
        self.peek_pos.push_back(0);
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
                .or_else(|| { error::error_exit(self.cur_line, "expcted macro args"); })
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
        MACRO_MAP.lock().unwrap().remove(mcro.val.as_str());
    }

    fn register_obj_macro(&mut self, name: String, body: Vec<Token>) {
        MACRO_MAP
            .lock()
            .unwrap()
            .insert(name, Macro::Object(body));
    }
    fn register_funclike_macro(&mut self, name: String, body: Vec<Token>) {
        MACRO_MAP
            .lock()
            .unwrap()
            .insert(name, Macro::FuncLike(body));
    }

    fn read_defined_op(&mut self) -> Token {
        // TODO: add err handler
        let mut tok = self.do_read_token().unwrap();
        if tok.val == "(" {
            tok = self.do_read_token().unwrap();
            self.expect_skip_symbol(Symbol::ClosingParen);
        }
        if MACRO_MAP.lock().unwrap().contains_key(tok.val.as_str()) {
            Token::new(TokenKind::IntNumber, "1", 0, self.cur_line)
        } else {
            Token::new(TokenKind::IntNumber, "0", 0, self.cur_line)
        }
    }
    fn read_intexpr_line(&mut self) -> Vec<Token> {
        let mut v: Vec<Token> = Vec::new();
        loop {
            let mut tok = self.do_read_token()
                .or_else(|| error::error_exit(self.cur_line, "expect a token, but reach EOF"))
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
                    v.push(Token::new(TokenKind::IntNumber, "0", 0, self.cur_line));
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

        self.unget(Token::new(TokenKind::Symbol(Symbol::Semicolon), "", 0, 0));
        self.unget_all(expr_line);

        let node = parser::Parser::new((*self).clone()).run_as_expr();

        self.buf.pop_back();

        node.show();
        println!();
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
            .or_else(|| error::error_exit(self.cur_line, "expected macro"))
            .unwrap()
            .val;
        self.do_read_if((*MACRO_MAP.lock().unwrap()).contains_key(mcro_name.as_str()));
    }
    fn read_ifndef(&mut self) {
        let mcro_name = self.do_read_token()
            .or_else(|| error::error_exit(self.cur_line, "expected macro"))
            .unwrap()
            .val;
        self.do_read_if(!(*MACRO_MAP.lock().unwrap()).contains_key(mcro_name.as_str()));
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
                .or_else(|| error::error_exit(lex.cur_line, "reach EOF"))
                .unwrap()
        };
        loop {
            if get_tok(self).val != "#" {
                continue;
            }

            let tok = get_tok(self);
            if nest == 0 {
                match tok.val.as_str() {
                    "else" | "elif" | "endif" => {
                        let line = self.cur_line;
                        self.unget(tok);
                        self.unget(Token::new(TokenKind::Identifier, "#", 0, line));
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
}
