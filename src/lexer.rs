use std::fs::OpenOptions;
use std::io::prelude::*;
use std::iter;
use std::str;
use std::collections::VecDeque;
use std::path;
use std::process;
use std::collections::HashSet;
use error;
use parser;
use MACRO_MAP;

#[derive(Debug)]
pub enum Macro {
    Object(Vec<Token>),
    FuncLike(Vec<String>, Vec<Token>), // args, body
}

#[derive(PartialEq, Debug, Clone)]
pub enum TokenKind {
    Identifier,
    IntNumber,
    FloatNumber,
    String,
    Char,
    Symbol,
    Newline,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub space: bool, // leading space
    pub val: String,
    pub hideset: HashSet<String>,
    pub line: i32,
}

impl Token {
    pub fn new(kind: TokenKind, val: &str, line: i32) -> Token {
        Token {
            kind: kind,
            space: false,
            val: val.to_string(),
            hideset: HashSet::new(),
            line: line,
        }
    }
}

pub struct Lexer<'a> {
    pub cur_line: i32,
    filename: String,
    peek: iter::Peekable<str::Chars<'a>>,
    peek_buf: VecDeque<char>,
    buf: VecDeque<Token>,
    cond_stack: Vec<bool>,
}

impl<'a> Lexer<'a> {
    pub fn new(filename: String, input: &'a str) -> Lexer<'a> {
        Lexer {
            cur_line: 1,
            filename: filename.to_string(),
            peek: input.chars().peekable(),
            peek_buf: VecDeque::new(),
            buf: VecDeque::new(),
            cond_stack: Vec::new(),
        }
    }
    pub fn get_filename(self) -> String {
        self.filename
    }
    fn peek_get(&mut self) -> Option<&char> {
        if self.peek_buf.len() > 0 {
            self.peek_buf.front()
        } else {
            self.peek.peek()
        }
    }
    fn peek_next(&mut self) -> char {
        if self.peek_buf.len() > 0 {
            self.peek_buf.pop_front().unwrap()
        } else {
            self.peek.next().unwrap()
        }
    }
    fn peek_unget(&mut self, ch: char) {
        self.peek_buf.push_back(ch);
    }
    fn peek_next_char_is(&mut self, ch: char) -> bool {
        let c = self.peek_next();
        let nextc = self.peek_next();
        self.peek_unget(c);
        self.peek_unget(nextc);
        nextc == ch
    }
    fn peek_char_is(&mut self, ch: char) -> bool {
        let line = self.cur_line;
        let errf =
            || -> Option<&char> { error::error_exit(line, format!("expected '{}'", ch).as_str()); };

        let peekc = self.peek_get().or_else(errf).unwrap();
        *peekc == ch
    }

    pub fn skip(&mut self, s: &str) -> bool {
        let next = self.read_token();
        match next {
            Some(n) => {
                if n.val == s && n.kind != TokenKind::String && n.kind != TokenKind::Char {
                    true
                } else {
                    self.buf.push_back(n);
                    false
                }
            }
            None => false,
        }
    }
    pub fn unget(&mut self, t: Token) {
        self.buf.push_back(t);
    }
    pub fn unget_all(&mut self, tv: Vec<Token>) {
        for t in tv {
            self.unget(t);
        }
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
            '+' | '-' => {
                if self.peek_char_is('=') || self.peek_char_is('+') || self.peek_char_is('-') {
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
        if !self.buf.is_empty() {
            return self.buf.pop_front();
        }

        match self.peek_get() {
            Some(&c) => {
                match c {
                    'a'...'z' | 'A'...'Z' | '_' => Some(self.read_identifier()),
                    ' ' | '\t' => {
                        self.peek_next();
                        // set a leading space
                        fn f(tok: Token) -> Option<Token> {
                            let mut t = tok;
                            t.space = true;
                            Some(t)
                        }
                        self.read_token().and_then(f)
                    }
                    '0'...'9' => Some(self.read_number_literal()),
                    '\"' => Some(self.read_string_literal()),
                    '\'' => Some(self.read_char_literal()),
                    '\n' => Some(self.read_newline()),
                    '\\' => {
                        self.peek_next();
                        self.read_token()
                    }
                    '/' => {
                        if self.peek_next_char_is('*') {
                            self.peek_next(); // /
                            self.peek_next(); // *
                            while !(self.peek_char_is('*') && self.peek_next_char_is('/')) {
                                self.peek_next();
                            }
                            self.peek_next();
                            self.peek_next();
                            self.do_read_token()
                        } else if self.peek_next_char_is('/') {
                            self.peek_next(); // /
                            self.peek_next(); // /
                            while !self.peek_char_is('\n') {
                                self.peek_next();
                            }
                            self.do_read_token()
                        } else {
                            Some(self.read_symbol())
                        }
                    }
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

    fn expand(&mut self, token: Option<Token>) -> Option<Token> {
        token.and_then(|tok| {
            let name = tok.val.clone();
            if tok.hideset.contains(tok.val.as_str()) {
                Some(tok)
            } else {
                // if cur token is macro:
                match MACRO_MAP.lock().unwrap().get(name.as_str()) {
                    Some(mcro) => {
                        match mcro {
                            &Macro::Object(ref body) => {
                                for t in body {
                                    self.unget(|| -> Token {
                                                   let mut a = t.clone();
                                                   a.hideset.insert(name.to_string());
                                                   a
                                               }());
                                }
                            }
                            _ => {}
                            // &Macro::FuncLike(ref m) => {}
                        }
                        self.read_token()
                    }
                    None => Some(tok),
                }
            }
        })
    }

    pub fn get(&mut self) -> Option<Token> {
        let t = self.read_token();
        let tok = match t {
            Some(tok) => {
                if tok.val == "#" {
                    // preprocessor directive
                    self.read_cpp_directive();
                    self.get()
                } else {
                    Some(tok)
                }
            }
            _ => return t,
        };
        self.expand(tok)
    }

    // for c preprocessor

    fn read_cpp_directive(&mut self) {
        let t = self.do_read_token(); // cpp directive
        match t.ok_or("error").unwrap().val.as_str() {
            "include" => self.read_include(),
            "define" => self.read_define(),
            "if" => self.read_if(),
            "ifdef" => self.read_ifdef(),
            "ifndef" => self.read_ifndef(),
            _ => {}
        }
    }

    fn try_include(&mut self, filename: &str) -> Option<String> {
        let header_paths = vec!["./include/",
                                "/include/",
                                "/usr/include/",
                                "/usr/include/linux/",
                                "/usr/include/x86_64-linux-gnu/",
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
    fn read_include(&mut self) {
        // this will be a function
        let mut filename = String::new();
        if self.skip("<") {
            while !self.peek_char_is('>') {
                filename.push(*self.peek_get().ok_or("error").unwrap());
                self.peek_next();
            }
            self.peek_next();
        }
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
        let mut lexer = Lexer::new(filename, body.as_str());
        loop {
            let t = lexer.get();
            match t {
                Some(tok) => self.buf.push_back(tok),
                None => break,
            }
        }
        println!("end of: {}", real_filename);
    }

    fn read_define(&mut self) {
        let mcro = self.do_read_token().unwrap();
        assert_eq!(mcro.kind, TokenKind::Identifier);

        // TODO: func like macro is unsupported now..
        if self.skip("(") {
            print!("\tmacro: {}(", mcro.val);
            // read macro arguments
            let mut args: Vec<String> = Vec::new();
            loop {
                let arg = self.get()
                    .or_else(|| { error::error_exit(self.cur_line, ""); })
                    .unwrap()
                    .val;
                args.push(arg);
                if self.skip(")") {
                    break;
                }
                self.skip(",");
            }
            for arg in args.clone() {
                print!("{},", arg);
            }
            println!(")");

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
            self.register_funclike_macro(mcro.val, args, body);
        } else {
            println!("\tmacro: {}", mcro.val);

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
            self.register_obj_macro(mcro.val, body);
        }
    }

    fn register_obj_macro(&mut self, name: String, body: Vec<Token>) {
        MACRO_MAP
            .lock()
            .unwrap()
            .insert(name, Macro::Object(body));
    }
    fn register_funclike_macro(&mut self, name: String, args: Vec<String>, body: Vec<Token>) {
        MACRO_MAP
            .lock()
            .unwrap()
            .insert(name, Macro::FuncLike(args, body));
    }

    fn read_defined_op(&mut self) -> Token {
        // TODO: add err handler
        self.skip("(");
        let tok = self.read_token().unwrap();
        self.skip(")");
        if MACRO_MAP.lock().unwrap().contains_key(tok.val.as_str()) {
            Token::new(TokenKind::IntNumber, "1", self.cur_line)
        } else {
            Token::new(TokenKind::IntNumber, "0", self.cur_line)
        }
    }
    fn read_intexpr_line(&mut self) -> Vec<Token> {
        let mut v: Vec<Token> = Vec::new();
        loop {
            let tok = self.do_read_token()
                .or_else(|| error::error_exit(self.cur_line, "expect a token, but reach EOF"))
                .unwrap();
            if tok.kind == TokenKind::Newline {
                break;
            } else if tok.val == "defined" {
                v.push(self.read_defined_op());
            } else if tok.kind == TokenKind::Identifier {
                // identifier in expr line is replaced with 0i
                v.push(Token::new(TokenKind::IntNumber, "0", self.cur_line));
            } else {
                v.push(tok);
            }
        }
        v
    }
    fn read_constexpr(&mut self) -> bool {
        let v = self.read_intexpr_line();
        self.unget_all(v);
        let node = parser::read_expr(self);
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
                        self.unget(Token::new(TokenKind::Symbol, "#", line));
                        self.unget(tok);
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
