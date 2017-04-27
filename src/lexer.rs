use std::fs::OpenOptions;
use std::io::prelude::*;
use std::iter;
use std::str;
use std::collections::VecDeque;
use std::path;
use std::process;
use std::collections::{HashSet, HashMap};
use error;
use parser;
use MACRO_MAP;

#[derive(Debug)]
pub enum Macro {
    Object(Vec<Token>),
    FuncLike(Vec<Token>), // args, body
}

#[derive(PartialEq, Debug, Clone)]
pub enum TokenKind {
    MacroParam,
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

pub struct Lexer<'a> {
    pub cur_line: i32,
    filename: String,
    peek: iter::Peekable<str::Chars<'a>>,
    peek_buf: VecDeque<char>,
    buf: VecDeque<VecDeque<Token>>,
    cond_stack: Vec<bool>,
}

impl<'a> Lexer<'a> {
    pub fn new(filename: String, input: &'a str) -> Lexer<'a> {
        let mut v: VecDeque<VecDeque<Token>> = VecDeque::new();
        v.push_back(VecDeque::new());
        Lexer {
            cur_line: 1,
            filename: filename.to_string(),
            peek: input.chars().peekable(),
            peek_buf: VecDeque::new(),
            buf: v,
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
        let next = self.do_read_token();
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
    pub fn expect_skip(&mut self, expect: &str) -> bool {
        if !self.skip(expect) {
            error::error_exit(self.cur_line, format!("expected '{}'", expect).as_str());
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
        Token::new(TokenKind::Identifier, ident.as_str(), 0, self.cur_line)
    }
    fn read_number_literal(&mut self) -> Token {
        let mut num = String::new();
        let mut is_float = false;
        loop {
            match self.peek_get() {
                Some(&c) => {
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
        Token::new(TokenKind::Symbol, sym.as_str(), 0, self.cur_line)
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
                        self.do_read_token().and_then(f)
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

    fn expand_obj_macro(&mut self, name: String, body: &Vec<Token>) {
        let mut bdy: Vec<Token> = Vec::new();
        for t in body {
            bdy.push(|| -> Token {
                         let mut a = t.clone();
                         a.hideset.insert(name.to_string());
                         a
                     }());
        }
        self.unget_all(bdy);
    }
    fn read_one_arg(&mut self) -> Vec<Token> {
        let mut n = 0;
        let mut arg: Vec<Token> = Vec::new();
        loop {
            let tok = self.read_token()
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
        // read macro arguments
        self.expect_skip("(");
        let mut args: Vec<Vec<Token>> = Vec::new();
        while !self.skip(")") {
            args.push(self.read_one_arg());
        }

        let mut expanded: Vec<Token> = Vec::new();
        let mut is_stringize = false;
        for macro_tok in macro_body {
            if macro_tok.val == "#" {
                is_stringize = true;
                continue;
            }
            if macro_tok.kind == TokenKind::MacroParam {
                let position = macro_tok.macro_position;
                if is_stringize {
                    expanded.push(self.stringize(&args[position]));
                    is_stringize = false;
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
            let name = tok.val.clone();

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
    fn read_include(&mut self) {
        // this will be a function
        let mut filename = String::new();
        if self.skip("<") {
            while !self.skip(">") {
                println!("{}", filename);
                filename.push_str(self.do_read_token().unwrap().val.as_str());
            }
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
        let mut v: Vec<Token> = Vec::new();
        loop {
            match lexer.get() {
                Some(tok) => v.push(tok),
                None => break,
            }
        }
        self.unget_all(v);
        println!("end of: {}", real_filename);
    }

    fn read_define(&mut self) {
        let mcro = self.do_read_token().unwrap();
        assert_eq!(mcro.kind, TokenKind::Identifier);

        let t = self.do_read_token().unwrap();
        let is_func_macro = if !t.space && t.val.as_str() == "(" {
            true
        } else {
            self.unget(t);
            false
        };
        if is_func_macro {
            print!("\tmacro: {}(", mcro.val);
            // read macro arguments
            let mut args: HashMap<String, usize> = HashMap::new();
            let mut count = 0usize;
            // TOD: not beautiful code :(
            if !self.skip(")") {
                loop {
                    let arg = self.get()
                        .or_else(|| { error::error_exit(self.cur_line, "expcted macro args"); })
                        .unwrap()
                        .val;
                    args.insert(arg, count);
                    if self.skip(")") {
                        break;
                    }
                    self.expect_skip(",");
                    count += 1;
                }
            }
            for (key, val) in args.clone() {
                print!("{}({}),", key, val);
            }
            println!(")");

            let mut body: Vec<Token> = Vec::new();
            print!("\tmacro body: ");
            loop {
                let tok = self.do_read_token().unwrap();
                if tok.kind == TokenKind::Newline {
                    break;
                }
                print!("{}{}", if tok.space { " " } else { "" }, tok.val);

                // if tok is a parameter of funclike macro,
                //  the kind of tok is changed to MacroParam
                //  and set macro_position
                let maybe_macro_name = tok.val.as_str();
                if args.contains_key(maybe_macro_name) {
                    let mut macro_param = tok.clone();
                    macro_param.kind = TokenKind::MacroParam;
                    macro_param.macro_position = args.get(maybe_macro_name).unwrap().clone();
                    body.push(macro_param);
                } else {
                    body.push(tok.clone());
                }
            }
            println!();
            self.register_funclike_macro(mcro.val, body);
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
            self.expect_skip(")");
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
            } else if tok.val == "defined" {
                v.push(self.read_defined_op());
            } else if tok.kind == TokenKind::Identifier {
                // identifier in expr line is replaced with 0i
                v.push(Token::new(TokenKind::IntNumber, "0", 0, self.cur_line));
            } else {
                v.push(tok);
            }
        }
        v
    }
    fn read_constexpr(&mut self) -> bool {
        let expr_line = self.read_intexpr_line();
        self.buf.push_back(VecDeque::new());

        self.unget(Token::new(TokenKind::Symbol, ";", 0, 0));
        self.unget_all(expr_line);
        println!("P:");
        for i in self.buf.back_mut().unwrap().clone() {
            println!(">{}", i.val);
        }
        let node = parser::read_expr(self);

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
                        self.unget(Token::new(TokenKind::Symbol, "#", 0, line));
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
