use lexer::{Keyword, Lexer, Pos, Symbol, Token, TokenKind};
use node::{ASTKind, Bits, AST};
use node;
use types::{Sign, StorageClass, Type};

use std::str;
use std::boxed::Box;
use std::io::{stderr, Write};
use std::collections::{hash_map, HashMap, VecDeque};

// use CODEGEN;

extern crate llvm_sys as llvm;

extern crate rand;
use self::rand::Rng;

extern crate ansi_term;
use self::ansi_term::Colour;

// TODO: add more error kinds
pub enum Error {
    Something,
    EOF,
}

pub struct Qualifiers {
    pub q_restrict: bool,
    pub q_const: bool,
    pub q_constexpr: bool,
    pub q_volatile: bool,
    pub q_inline: bool,
    pub q_noreturn: bool,
}

impl Qualifiers {
    pub fn new() -> Qualifiers {
        Qualifiers {
            q_restrict: false,
            q_const: false,
            q_constexpr: false,
            q_volatile: false,
            q_inline: false,
            q_noreturn: false,
        }
    }
}

pub type ParseR<T> = Result<T, Error>;

pub struct Parser<'a> {
    pub lexer: &'a mut Lexer,
    pub err_counts: usize,
    env: Env<AST>,
    tags: Env<Type>,
    // constexpr_func_map: HashSet<String>,
}

pub struct Env<T: Clone>(pub VecDeque<HashMap<String, T>>);

impl<T: Clone> Env<T> {
    fn new() -> Env<T> {
        let mut env = VecDeque::new();
        env.push_back(HashMap::new());
        Env(env)
    }
    fn push(&mut self) {
        let localenv = (*self.0.back().unwrap()).clone();
        self.0.push_back(localenv);
    }
    fn pop(&mut self) {
        self.0.pop_back();
    }
    fn add(&mut self, name: String, val: T) {
        self.0.back_mut().unwrap().insert(name, val);
    }
    fn add_globally(&mut self, name: String, val: T) {
        self.0[0].insert(name.clone(), val.clone());
        self.0.back_mut().unwrap().insert(name, val);
    }
    fn is_local(&self) -> bool {
        self.0.len() > 1
    }
    fn back_mut(&mut self) -> Option<&mut HashMap<String, T>> {
        self.0.back_mut()
    }
    fn get(&mut self, name: &str) -> Option<&T> {
        self.0.back_mut().unwrap().get(name)
    }
    fn contains(&mut self, name: &str) -> bool {
        self.0.back_mut().unwrap().contains_key(name)
    }
}

macro_rules! matches {
    ($e:expr, $p:pat) => {
        match $e {
            $p => true,
            _ => false
        }
    }
}
macro_rules! ident_val {
    ($e:expr) => {
        match &$e.kind {
            &TokenKind::Identifier(ref ident) => ident.to_string(),
            _ => "".to_string()
        }
    }
}
macro_rules! expect_symbol_error {
    ($slf:expr, $sym:expr, $msg:expr) => {{
        if !try!($slf.lexer.skip_symbol($sym)) {
            let peek = $slf.lexer.peek();
            $slf.show_error_token(&try!(peek), $msg);
        }
    }}
}

impl<'a> Parser<'a> {
    pub fn new(lexer: &'a mut Lexer) -> Parser<'a> {
        Parser {
            lexer: lexer,
            err_counts: 0,
            env: Env::new(),
            tags: Env::new(),
            // constexpr_func_map: HashSet::new(),
        }
    }
    fn show_error(&mut self, msg: &str) {
        self.err_counts += 1;
        writeln!(
            &mut stderr(),
            "{}: {} {}: {}",
            self.lexer.get_filename(),
            Colour::Red.bold().paint("error:"),
            self.lexer.get_cur_line(),
            msg
        ).unwrap();
    }
    fn show_error_token(&mut self, token: &Token, msg: &str) {
        self.err_counts += 1;
        writeln!(
            &mut stderr(),
            "{}: {} {}: {}",
            self.lexer.get_filename(),
            Colour::Red.bold().paint("error:"),
            token.pos.line,
            msg
        ).unwrap();
        writeln!(
            &mut stderr(),
            "{}",
            self.lexer
                .get_surrounding_code_with_err_point(token.pos.pos,)
        ).unwrap();
    }
    pub fn run_file(filename: String) -> Vec<AST> {
        let mut nodes: Vec<AST> = Vec::new();
        let mut lexer = Lexer::new(filename.to_string());
        // TODO: for debugging
        // loop {
        //     let tok = lexer.get();
        //     match tok {
        //         Some(t) => {
        //             println!("t:{}{:?} {}", if t.space { " " } else { "" }, t.kind, t.val);
        //         }
        //         None => break,
        //     }
        // }
        //
        // // Debug: (parsing again is big cost?)
        // lexer = Lexer::new(filename.to_string(), s.as_str());
        Parser::new(&mut lexer).run(&mut nodes);
        nodes
    }
    pub fn run(&mut self, node: &mut Vec<AST>) {
        while matches!(self.read_toplevel(node), Ok(_)) {}
        self.show_total_errors();
    }
    pub fn run_as_expr(&mut self) -> ParseR<AST> {
        self.read_expr()
    }
    pub fn show_total_errors(&mut self) {
        if self.err_counts > 0 {
            println!(
                "{} error{} generated.",
                self.err_counts,
                if self.err_counts > 1 { "s" } else { "" }
            );
            ::std::process::exit(-1);
        }
    }
    pub fn read_toplevel(&mut self, ast: &mut Vec<AST>) -> ParseR<()> {
        // TODO: refine
        if try!(self.is_function_def()) {
            match self.read_func_def() {
                Ok(ok) => ast.push(ok),
                Err(Error::EOF) => self.show_error("expected a token, but reached EOF"),
                Err(e) => return Err(e),
            }
        } else {
            match self.read_decl(ast) {
                Err(Error::EOF) => self.show_error("expected a token, but reached EOF"),
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
    fn read_func_def(&mut self) -> ParseR<AST> {
        self.env.push();
        self.tags.push();

        let (ret_ty, _, _qualifiers) = try!(self.read_type_spec());
        let (functy, name, param_names) = try!(self.read_declarator(ret_ty));

        // if qualifiers.q_constexpr {
        //     self.constexpr_func_map.insert(name.clone());
        // }

        self.env.add_globally(
            name.clone(),
            AST::new(
                ASTKind::Variable(functy.clone(), name.clone()),
                Pos::new(0, 0),
            ),
        );
        self.env.add(
            "__func__".to_string(),
            AST::new(ASTKind::String(name.clone()), Pos::new(0, 0)),
        );

        expect_symbol_error!(self, Symbol::OpeningBrace, "expected '('");
        let body = try!(self.read_func_body(&functy));

        self.env.pop();
        self.tags.pop();

        Ok(AST::new(
            ASTKind::FuncDef(
                functy,
                if param_names.is_none() {
                    Vec::new()
                } else {
                    param_names.unwrap()
                },
                name,
                Box::new(body),
            ),
            Pos::new(0, 0),
        ))
    }
    fn read_func_body(&mut self, _functy: &Type) -> ParseR<AST> {
        self.read_compound_stmt()
    }
    fn read_compound_stmt(&mut self) -> ParseR<AST> {
        let mut stmts: Vec<AST> = Vec::new();
        loop {
            if try!(self.lexer.skip_symbol(Symbol::ClosingBrace).or_else(|eof| {
                self.show_error("expected '}'");
                Err(eof)
            })) {
                break;
            }

            let peek_tok = try!(self.lexer.peek());
            if self.is_type(&peek_tok) {
                // variable declaration
                try!(self.read_decl(&mut stmts));
            } else {
                match self.read_stmt() {
                    Ok(stmt) => stmts.push(stmt),
                    Err(_) => {}
                }
            }
        }
        Ok(AST::new(ASTKind::Block(stmts), Pos::new(0, 0)))
    }
    fn read_stmt(&mut self) -> ParseR<AST> {
        let tok = try!(self.lexer.get());
        if let &TokenKind::Keyword(ref keyw) = &tok.kind {
            match *keyw {
                Keyword::If => return self.read_if_stmt(),
                Keyword::For => return self.read_for_stmt(),
                Keyword::While => return self.read_while_stmt(),
                Keyword::Do => return self.read_do_while_stmt(),
                Keyword::Switch => return self.read_switch_stmt(),
                Keyword::Case => return self.read_case_label(),
                Keyword::Default => return self.read_default_label(),
                Keyword::Goto => return self.read_goto_stmt(),
                Keyword::Continue => return self.read_continue_stmt(),
                Keyword::Break => return self.read_break_stmt(),
                Keyword::Return => return self.read_return_stmt(),
                _ => {}
            }
        } else if let &TokenKind::Symbol(Symbol::OpeningBrace) = &tok.kind {
            return self.read_compound_stmt();
        }

        if matches!(tok.kind, TokenKind::Identifier(_))
            && try!(self.lexer.peek_symbol_token_is(Symbol::Colon))
        {
            return self.read_label(tok);
        }

        self.lexer.unget(tok);
        let expr = self.read_opt_expr();
        expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
        expr
    }
    fn read_if_stmt(&mut self) -> ParseR<AST> {
        expect_symbol_error!(self, Symbol::OpeningParen, "expected '('");
        let cond = try!(self.read_expr());
        expect_symbol_error!(self, Symbol::ClosingParen, "expected ')'");
        let then_stmt = Box::new(try!(self.read_stmt()));
        let else_stmt = if try!(self.lexer.skip_keyword(Keyword::Else)) {
            Box::new(try!(self.read_stmt()))
        } else {
            Box::new(AST::new(ASTKind::Block(Vec::new()), Pos::new(0, 0)))
        };
        Ok(AST::new(
            ASTKind::If(Box::new(cond), then_stmt, else_stmt),
            Pos::new(0, 0),
        ))
    }
    fn read_for_stmt(&mut self) -> ParseR<AST> {
        expect_symbol_error!(self, Symbol::OpeningParen, "expected '('");
        let init = try!(self.read_opt_decl_or_stmt());
        // TODO: make read_expr return Option<AST>.
        //       when cur tok is ';', returns None.
        let cond = try!(self.read_opt_expr());
        expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
        let step = if try!(self.lexer.peek_symbol_token_is(Symbol::ClosingParen)) {
            AST::new(ASTKind::Compound(Vec::new()), self.lexer.get_cur_pos())
        } else {
            try!(self.read_opt_expr())
        };
        expect_symbol_error!(self, Symbol::ClosingParen, "expected ')'");
        let body = try!(self.read_stmt());
        Ok(AST::new(
            ASTKind::For(
                Box::new(init),
                Box::new(cond),
                Box::new(step),
                Box::new(body),
            ),
            Pos::new(0, 0),
        ))
    }
    fn read_while_stmt(&mut self) -> ParseR<AST> {
        expect_symbol_error!(self, Symbol::OpeningParen, "expected '('");
        let cond = try!(self.read_expr());
        expect_symbol_error!(self, Symbol::ClosingParen, "expected ')'");
        let body = try!(self.read_stmt());
        Ok(AST::new(
            ASTKind::While(Box::new(cond), Box::new(body)),
            Pos::new(0, 0),
        ))
    }
    fn read_do_while_stmt(&mut self) -> ParseR<AST> {
        let body = try!(self.read_stmt());
        if !try!(self.lexer.skip_keyword(Keyword::While)) {
            let peek = self.lexer.peek();
            self.show_error_token(&try!(peek), "expected 'while'");
        }
        expect_symbol_error!(self, Symbol::OpeningParen, "expected '('");
        let cond = try!(self.read_expr());
        expect_symbol_error!(self, Symbol::ClosingParen, "expected ')'");
        expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
        Ok(AST::new(
            ASTKind::DoWhile(Box::new(cond), Box::new(body)),
            Pos::new(0, 0),
        ))
    }
    fn read_switch_stmt(&mut self) -> ParseR<AST> {
        expect_symbol_error!(self, Symbol::OpeningParen, "expected '('");
        let cond = try!(self.read_expr());
        expect_symbol_error!(self, Symbol::ClosingParen, "expected ')'");
        let body = Box::new(try!(self.read_stmt()));
        Ok(AST::new(
            ASTKind::Switch(Box::new(cond), body),
            Pos::new(0, 0),
        ))
    }
    fn read_case_label(&mut self) -> ParseR<AST> {
        let expr = try!(self.read_expr());
        expect_symbol_error!(self, Symbol::Colon, "expected ':'");
        Ok(AST::new(ASTKind::Case(Box::new(expr)), Pos::new(0, 0)))
    }
    fn read_default_label(&mut self) -> ParseR<AST> {
        expect_symbol_error!(self, Symbol::Colon, "expected ':'");
        Ok(AST::new(ASTKind::DefaultL, Pos::new(0, 0)))
    }
    fn read_goto_stmt(&mut self) -> ParseR<AST> {
        let pos = self.lexer.get_cur_pos();
        let label_name = ident_val!(try!(self.lexer.get()));
        expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
        Ok(AST::new(ASTKind::Goto(label_name), pos))
    }
    fn read_label(&mut self, tok: Token) -> ParseR<AST> {
        let pos = self.lexer.get_cur_pos();
        let label_name = ident_val!(tok);
        expect_symbol_error!(self, Symbol::Colon, "expected ':'");
        Ok(AST::new(ASTKind::Label(label_name), pos))
    }
    fn read_continue_stmt(&mut self) -> ParseR<AST> {
        let pos = self.lexer.get_cur_pos();
        expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
        Ok(AST::new(ASTKind::Continue, pos))
    }
    fn read_break_stmt(&mut self) -> ParseR<AST> {
        let pos = self.lexer.get_cur_pos();
        expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
        Ok(AST::new(ASTKind::Break, pos))
    }
    fn read_return_stmt(&mut self) -> ParseR<AST> {
        let pos = self.lexer.get_cur_pos();
        if try!(self.lexer.skip_symbol(Symbol::Semicolon)) {
            Ok(AST::new(ASTKind::Return(None), pos))
        } else {
            let retval = Some(Box::new(try!(self.read_expr())));
            expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
            Ok(AST::new(ASTKind::Return(retval), pos))
        }
    }
    fn is_function_def(&mut self) -> ParseR<bool> {
        let mut buf = Vec::new();
        let mut is_funcdef = false;

        loop {
            let mut tok = try!(self.lexer.get());
            buf.push(tok.clone());

            if tok.kind == TokenKind::Symbol(Symbol::Semicolon) {
                break;
            }

            if self.is_type(&tok) {
                continue;
            }

            if tok.kind == TokenKind::Symbol(Symbol::OpeningParen) {
                try!(self.skip_parens(&tok, &mut buf));
                continue;
            }

            if !matches!(tok.kind, TokenKind::Identifier(_)) {
                continue;
            }

            if try!(self.lexer.peek()).kind != TokenKind::Symbol(Symbol::OpeningParen) {
                continue;
            }

            let opening_paren = try!(self.lexer.get());
            buf.push(opening_paren.clone());
            try!(self.skip_parens(&opening_paren, &mut buf));

            tok = try!(self.lexer.peek());
            is_funcdef = tok.kind == TokenKind::Symbol(Symbol::OpeningBrace);
            break;
        }

        self.lexer.unget_all(&buf);
        Ok(is_funcdef)
    }
    fn skip_parens(&mut self, opening_paren: &Token, buf: &mut Vec<Token>) -> ParseR<()> {
        loop {
            let tok = try!(self.lexer.get().or_else(|_| {
                self.show_error_token(&opening_paren, "expected ')', but reach EOF");
                return Err(Error::Something);
            }));
            buf.push(tok.clone());

            match tok.kind {
                TokenKind::Symbol(Symbol::OpeningParen) => try!(self.skip_parens(&tok, buf)),
                TokenKind::Symbol(Symbol::ClosingParen) => break,
                _ => {}
            };
        }
        Ok(())
    }
    fn skip_until(&mut self, sym: Symbol) {
        let ts = TokenKind::Symbol(sym);
        while match self.lexer.get() {
            Ok(tok) => tok.kind != ts,
            Err(_) => false,
        } {}
    }

    fn get_typedef(&mut self, name: &str) -> ParseR<Option<Type>> {
        match self.env.get(name) {
            Some(ast) => match ast.kind {
                ASTKind::Typedef(ref from, ref _to) => {
                    let ty = match from {
                        &Type::Struct(ref name, ref fields)
                        | &Type::Union(ref name, ref fields, _) => {
                            if fields.is_empty() {
                                self.tags.get(name.as_str()).unwrap().clone()
                            } else {
                                from.clone()
                            }
                        }
                        _ => from.clone(),
                    };
                    return Ok(Some(ty));
                }
                _ => {}
            },
            None => return Ok(None),
        }
        Ok(None)
    }
    fn is_type(&mut self, token: &Token) -> bool {
        if let TokenKind::Keyword(ref keyw) = token.kind {
            match *keyw {
                Keyword::Typedef
                | Keyword::Extern
                | Keyword::Static
                | Keyword::Auto
                | Keyword::Register
                | Keyword::Const
                | Keyword::Volatile
                | Keyword::Void
                | Keyword::Signed
                | Keyword::Unsigned
                | Keyword::Char
                | Keyword::Int
                | Keyword::Short
                | Keyword::Long
                | Keyword::Float
                | Keyword::Double
                | Keyword::Struct
                | Keyword::Enum
                | Keyword::Union
                | Keyword::Noreturn
                | Keyword::Inline
                | Keyword::Restrict => true,
                _ => false,
            }
        } else if let TokenKind::Identifier(ref ident) = token.kind {
            match self.env.get(ident.as_str()) {
                Some(ast) => match ast.kind {
                    ASTKind::Typedef(_, _) => true,
                    _ => false,
                },
                None => false,
            }
        } else {
            false
        }
    }
    fn is_string(&self, ty: &Type) -> bool {
        if let &Type::Array(ref elem_ty, _) = ty {
            if matches!(**elem_ty, Type::Char(Sign::Signed)) {
                return true;
            }
        }
        false
    }
    fn read_decl_init(&mut self, ty: &mut Type) -> ParseR<AST> {
        // TODO: implement for like 'int a[] = {...}, char *s="str";'
        if try!(self.lexer.peek_symbol_token_is(Symbol::OpeningBrace)) {
            return self.read_initializer_list(ty);
        } else if self.is_string(ty) {
            let tok = try!(self.lexer.get());
            if let TokenKind::String(s) = tok.kind {
                return self.read_string_initializer(ty, s);
            }
            self.lexer.unget(tok);
        }
        self.read_assign()
    }
    fn read_initializer_elem(&mut self, ty: &mut Type) -> ParseR<AST> {
        if match *ty {
            Type::Array(_, _) | Type::Struct(_, _) | Type::Union(_, _, _) => true,
            _ => false,
        } {
            self.read_initializer_list(ty)
        } else if try!(self.lexer.peek_symbol_token_is(Symbol::OpeningBrace)) {
            let elem = self.read_initializer_elem(ty);
            expect_symbol_error!(self, Symbol::ClosingBrace, "expected '}'");
            elem
        } else {
            self.read_assign()
        }
    }
    fn read_initializer_list(&mut self, ty: &mut Type) -> ParseR<AST> {
        if self.is_string(ty) {
            let tok = try!(self.lexer.get());
            if let TokenKind::String(s) = tok.kind {
                return self.read_string_initializer(ty, s);
            }
            self.lexer.unget(tok);
        }
        match ty {
            &mut Type::Array(_, _) => self.read_array_initializer(ty),
            &mut Type::Struct(_, _) | &mut Type::Union(_, _, _) => self.read_struct_initializer(ty),
            _ => self.read_assign(),
        }
    }
    fn read_string_initializer(&mut self, ty: &mut Type, string: String) -> ParseR<AST> {
        let char_ary = string
            .chars()
            .map(|c| AST::new(ASTKind::Char(c as i32), Pos::new(0, 0)))
            .collect::<Vec<AST>>();
        if let &mut Type::Array(_, ref mut len) = ty {
            *len = char_ary.len() as i32 + 1;
        } else {
            panic!()
        }
        Ok(AST::new(
            ASTKind::ConstArray(char_ary),
            self.lexer.get_cur_pos(),
        ))
    }
    fn read_array_initializer(&mut self, ty: &mut Type) -> ParseR<AST> {
        let has_brace = try!(self.lexer.skip_symbol(Symbol::OpeningBrace));

        if let &mut Type::Array(ref elem_ty, ref mut len) = ty {
            let is_flexible = *len < 0;
            let mut elems = Vec::new();
            let mut elem_ty = (**elem_ty).clone();
            loop {
                let tok = try!(self.lexer.get());
                if let TokenKind::Symbol(Symbol::ClosingBrace) = tok.kind {
                    if !has_brace {
                        self.lexer.unget(tok);
                    }
                    break;
                }
                self.lexer.unget(tok);
                let elem = try!(self.read_initializer_elem((&mut elem_ty)));
                elems.push(elem);
                try!(self.lexer.skip_symbol(Symbol::Comma));
            }
            if is_flexible {
                *len = elems.len() as i32;
            }
            Ok(AST::new(
                ASTKind::ConstArray(elems),
                self.lexer.get_cur_pos(),
            ))
        } else {
            panic!()
        }
    }
    fn read_struct_initializer(&mut self, ty: &mut Type) -> ParseR<AST> {
        let tok = try!(self.lexer.get());
        let has_brace = tok.kind == TokenKind::Symbol(Symbol::OpeningBrace);

        let mut fields_types = if let Some(fields_types) = ty.get_all_fields_types() {
            fields_types
        } else {
            self.show_error_token(&tok, "initializer of struct must be array");
            return Err(Error::Something);
        };

        let mut elems = Vec::new();
        let mut field_type = fields_types.iter_mut();
        loop {
            let tok = try!(self.lexer.get());
            if let TokenKind::Symbol(Symbol::ClosingBrace) = tok.kind {
                if !has_brace {
                    self.lexer.unget(tok);
                }
                break;
            }
            self.lexer.unget(tok);
            let elem = try!(self.read_initializer_elem(&mut field_type.next().unwrap().clone()));
            elems.push(elem);
            try!(self.lexer.skip_symbol(Symbol::Comma));
        }
        Ok(AST::new(
            ASTKind::ConstStruct(elems),
            self.lexer.get_cur_pos(),
        ))
    }
    fn skip_type_qualifiers(&mut self) -> ParseR<()> {
        while try!(self.lexer.skip_keyword(Keyword::Const))
            || try!(self.lexer.skip_keyword(Keyword::Volatile))
            || try!(self.lexer.skip_keyword(Keyword::Restrict))
        {}
        Ok(())
    }
    fn read_decl(&mut self, ast: &mut Vec<AST>) -> ParseR<()> {
        let (basety, sclass, qualifiers) = try!(self.read_type_spec());
        let is_typedef = sclass == StorageClass::Typedef;

        if try!(self.lexer.skip_symbol(Symbol::Semicolon)) {
            return Ok(());
        }

        loop {
            let (mut ty, name, _) = try!(self.read_declarator(basety.clone())); // XXX

            if (qualifiers.q_constexpr || qualifiers.q_const)
                && try!(self.lexer.skip_symbol(Symbol::Assign))
            {
                let init = try!(self.read_decl_init(&mut ty));
                self.env.add(name.clone(), init);
            } else {
                if is_typedef {
                    let typedef = AST::new(
                        ASTKind::Typedef(ty, name.to_string()),
                        self.lexer.get_cur_pos(),
                    );
                    self.env.add(name, typedef);
                    return Ok(());
                }

                let init = if try!(self.lexer.skip_symbol(Symbol::Assign)) {
                    Some(Box::new(try!(self.read_decl_init(&mut ty))))
                } else {
                    None
                };
                self.env.add(
                    name.clone(),
                    AST::new(ASTKind::Variable(ty.clone(), name.clone()), Pos::new(0, 0)),
                );
                ast.push(AST::new(
                    ASTKind::VariableDecl(ty, name, sclass.clone(), init),
                    self.lexer.get_cur_pos(),
                ));
            }

            if try!(self.lexer.skip_symbol(Symbol::Semicolon)) {
                return Ok(());
            }
            if !try!(self.lexer.skip_symbol(Symbol::Comma)) {
                let peek = try!(self.lexer.get());
                self.show_error_token(&peek, "expected ','");
                self.skip_until(Symbol::Semicolon);
                return Err(Error::Something);
            }
        }
    }
    fn read_opt_decl_or_stmt(&mut self) -> ParseR<AST> {
        if try!(self.lexer.skip_symbol(Symbol::Semicolon)) {
            return Ok(AST::new(ASTKind::Compound(Vec::new()), Pos::new(0, 0)));
        }

        let peek_tok = try!(self.lexer.peek());
        if self.is_type(&peek_tok) {
            // variable declaration
            let mut stmts = Vec::new();
            let pos = self.lexer.get_cur_pos();
            try!(self.read_decl(&mut stmts));
            Ok(AST::new(ASTKind::Compound(stmts), pos))
        } else {
            self.read_stmt()
        }
    }
    // returns (declarator type, name, params{for function})
    fn read_declarator(&mut self, basety: Type) -> ParseR<(Type, String, Option<Vec<String>>)> {
        if try!(self.lexer.skip_symbol(Symbol::OpeningParen)) {
            let peek_tok = try!(self.lexer.peek());
            if self.is_type(&peek_tok) {
                let (ty, params) = try!(self.read_declarator_func(basety));
                return Ok((ty, "".to_string(), params));
            }

            // TODO: HUH? MAKES NO SENSE!!
            let mut buf: Vec<Token> = Vec::new();
            while !try!(self.lexer.skip_symbol(Symbol::ClosingParen)) {
                buf.push(try!(self.lexer.get()));
            }
            let t = try!(self.read_declarator_tail(basety));
            self.lexer.unget_all(&buf);
            return self.read_declarator(t.0);
        }

        if try!(self.lexer.skip_symbol(Symbol::Asterisk)) {
            try!(self.skip_type_qualifiers());
            return self.read_declarator(Type::Ptr(Box::new(basety.clone())));
        }

        let tok = try!(self.lexer.get());

        if let &TokenKind::Identifier(ref name) = &tok.kind {
            let (ty, params) = try!(self.read_declarator_tail(basety));
            return Ok((ty, name.to_string(), params));
        }

        self.lexer.unget(tok);
        let (ty, params) = try!(self.read_declarator_tail(basety));
        Ok((ty, "".to_string(), params))
    }
    fn read_declarator_tail(&mut self, basety: Type) -> ParseR<(Type, Option<Vec<String>>)> {
        if try!(self.lexer.skip_symbol(Symbol::OpeningBoxBracket)) {
            return Ok((try!(self.read_declarator_array(basety)), None));
        }
        if try!(self.lexer.skip_symbol(Symbol::OpeningParen)) {
            return self.read_declarator_func(basety);
        }
        Ok((basety, None))
    }

    fn read_declarator_array(&mut self, basety: Type) -> ParseR<Type> {
        let len: i32;
        if try!(self.lexer.skip_symbol(Symbol::ClosingBoxBracket)) {
            len = -1;
        } else {
            len = match try!(self.read_expr()).eval_constexpr() {
                Ok(len) => len as i32,
                Err(Error::Something) => {
                    let peek = try!(self.lexer.peek());
                    self.show_error_token(&peek, "array size must be constant");
                    0
                }
                Err(e) => return Err(e),
            };
            expect_symbol_error!(self, Symbol::ClosingBoxBracket, "expected ']'");
        }
        let ty = try!(self.read_declarator_tail(basety)).0;
        Ok(Type::Array(Box::new(ty), len))
    }
    fn read_declarator_func(&mut self, retty: Type) -> ParseR<(Type, Option<Vec<String>>)> {
        if try!(self.lexer.peek_keyword_token_is(Keyword::Void))
            && try!(self.lexer.next_symbol_token_is(Symbol::ClosingParen))
        {
            try!(self.lexer.expect_skip_keyword(Keyword::Void));
            try!(self.lexer.expect_skip_symbol(Symbol::ClosingParen));
            return Ok((Type::Func(Box::new(retty), Vec::new(), false), None));
        }
        if try!(self.lexer.skip_symbol(Symbol::ClosingParen)) {
            return Ok((Type::Func(Box::new(retty), Vec::new(), false), None));
        }

        let (paramtypes, paramnames, vararg) = try!(self.read_declarator_params());
        Ok((
            Type::Func(Box::new(retty), paramtypes, vararg),
            Some(paramnames),
        ))
    }
    // returns (param types, param names, vararg?)
    fn read_declarator_params(&mut self) -> ParseR<(Vec<Type>, Vec<String>, bool)> {
        let mut paramtypes: Vec<Type> = Vec::new();
        let mut paramnames: Vec<String> = Vec::new();
        loop {
            if try!(self.lexer.skip_symbol(Symbol::Vararg)) {
                if paramtypes.len() == 0 {
                    let peek = self.lexer.peek();
                    self.show_error_token(
                        &try!(peek),
                        "at least one param is required before '...'",
                    );
                    return Err(Error::Something);
                }
                expect_symbol_error!(self, Symbol::ClosingParen, "expected ')'");
                return Ok((paramtypes, paramnames, true));
            }

            let (ty, name) = try!(self.read_func_param());

            // if reading a parameter of a function to define
            if self.env.is_local() {
                self.env.add(
                    name.clone(),
                    AST::new(ASTKind::Variable(ty.clone(), name.clone()), Pos::new(0, 0)),
                );
            }
            paramtypes.push(ty);
            paramnames.push(name);
            if try!(self.lexer.skip_symbol(Symbol::ClosingParen)) {
                return Ok((paramtypes, paramnames, false));
            }
            if !try!(self.lexer.skip_symbol(Symbol::Comma)) {
                let peek = self.lexer.peek();
                self.show_error_token(&try!(peek), "expected ','");
                self.skip_until(Symbol::ClosingParen);
                return Err(Error::Something);
            }
        }
    }
    fn read_func_param(&mut self) -> ParseR<(Type, String)> {
        let basety = try!(self.read_type_spec()).0;
        let (ty, name, _) = try!(self.read_declarator(basety));
        match ty {
            Type::Array(subst, _) => Ok((Type::Ptr(subst), name)),
            Type::Func(_, _, _) => Ok((Type::Ptr(Box::new(ty)), name)),
            _ => Ok((ty, name)),
        }
    }
    fn read_type_spec(&mut self) -> ParseR<(Type, StorageClass, Qualifiers)> {
        #[derive(PartialEq, Debug, Clone)]
        enum Size {
            Short,
            Normal,
            Long,
            LLong,
        };
        #[derive(PartialEq, Debug, Clone)]
        enum PrimitiveType {
            Void,
            Char,
            Int,
            Float,
            Double,
        };

        let mut kind: Option<PrimitiveType> = None;
        let mut sign: Option<Sign> = None;
        let mut size = Size::Normal;
        let mut sclass = StorageClass::Auto;
        let mut userty: Option<Type> = None;
        let mut qualifiers = Qualifiers::new();

        loop {
            let tok = try!(self.lexer.get());

            if kind.is_none() {
                if let &TokenKind::Identifier(ref maybe_userty_name) = &tok.kind {
                    let maybe_userty = try!(self.get_typedef(maybe_userty_name));
                    if maybe_userty.is_some() {
                        return Ok((maybe_userty.unwrap(), sclass, qualifiers));
                    }
                }
            }
            if !matches!(tok.kind, TokenKind::Keyword(_)) {
                self.lexer.unget(tok);
                break;
            }

            if let TokenKind::Keyword(keyw) = tok.kind {
                match &keyw {
                    &Keyword::Typedef => sclass = StorageClass::Typedef,
                    &Keyword::Extern => sclass = StorageClass::Extern,
                    &Keyword::Static => sclass = StorageClass::Static,
                    &Keyword::Auto => sclass = StorageClass::Auto,
                    &Keyword::Register => sclass = StorageClass::Register,
                    &Keyword::Const => qualifiers.q_const = true,
                    &Keyword::ConstExpr => qualifiers.q_constexpr = true,
                    &Keyword::Volatile => qualifiers.q_volatile = true,
                    &Keyword::Inline => qualifiers.q_inline = true,
                    &Keyword::Restrict => qualifiers.q_restrict = true,
                    &Keyword::Noreturn => qualifiers.q_noreturn = true,
                    &Keyword::Void => {
                        if kind.is_some() {
                            let peek = self.lexer.peek();
                            self.show_error_token(&try!(peek), "type mismatch");
                        }
                        kind = Some(PrimitiveType::Void);
                    }
                    &Keyword::Char => {
                        if kind.is_some() {
                            let peek = self.lexer.peek();
                            self.show_error_token(&try!(peek), "type mismatch");
                        }
                        kind = Some(PrimitiveType::Char);
                    }
                    &Keyword::Int => {
                        if kind.is_some() {
                            let peek = self.lexer.peek();
                            self.show_error_token(&try!(peek), "type mismatch");
                        }
                        kind = Some(PrimitiveType::Int);
                    }
                    &Keyword::Float => {
                        if kind.is_some() {
                            let peek = self.lexer.peek();
                            self.show_error_token(&try!(peek), "type mismatch");
                        }
                        kind = Some(PrimitiveType::Float);
                    }
                    &Keyword::Double => {
                        if kind.is_some() {
                            let peek = self.lexer.peek();
                            self.show_error_token(&try!(peek), "type mismatch");
                        }
                        kind = Some(PrimitiveType::Double);
                    }
                    &Keyword::Signed => {
                        if sign.is_some() {
                            let peek = self.lexer.peek();
                            self.show_error_token(&try!(peek), "type mismatch");
                        };

                        sign = Some(Sign::Signed);
                    }
                    &Keyword::Unsigned => {
                        if sign.is_some() {
                            let peek = self.lexer.peek();
                            self.show_error_token(&try!(peek), "type mismatch");
                        };

                        sign = Some(Sign::Unsigned);
                    }
                    &Keyword::Short => size = Size::Short,
                    &Keyword::Long => {
                        if size == Size::Normal {
                            size = Size::Long;
                        } else if size == Size::Long {
                            size = Size::LLong;
                        }
                    }
                    &Keyword::Struct => userty = Some(try!(self.read_struct_def())),
                    &Keyword::Union => userty = Some(try!(self.read_union_def())),
                    &Keyword::Enum => userty = Some(try!(self.read_enum_def())),
                    _ => {}
                }
            } else {
                self.lexer.unget(tok);
                break;
            }
        }

        // if sign is not expected,
        //  default is Signed
        if sign.is_none() {
            sign = Some(Sign::Signed);
        }

        // TODO: add err handler
        if userty.is_some() {
            return Ok((userty.unwrap(), sclass, qualifiers));
        }

        if kind.is_some() {
            match kind.unwrap() {
                PrimitiveType::Void => return Ok((Type::Void, sclass, qualifiers)),
                PrimitiveType::Char => return Ok((Type::Char(sign.unwrap()), sclass, qualifiers)),
                PrimitiveType::Float => return Ok((Type::Float, sclass, qualifiers)),
                PrimitiveType::Double => return Ok((Type::Double, sclass, qualifiers)),
                _ => {}
            }
        }

        let ty = match size {
            Size::Short => Type::Short(sign.unwrap()),
            Size::Normal => Type::Int(sign.unwrap()),
            Size::Long => Type::Long(sign.unwrap()),
            Size::LLong => Type::LLong(sign.unwrap()),
        };

        Ok((ty, sclass, qualifiers))
    }

    fn read_struct_def(&mut self) -> ParseR<Type> {
        self.read_rectype_def(true)
    }
    fn read_union_def(&mut self) -> ParseR<Type> {
        self.read_rectype_def(false)
    }
    // rectype is abbreviation of 'record type'
    fn read_rectype_tag(&mut self) -> ParseR<Option<String>> {
        let maybe_tag = try!(self.lexer.get());
        if let TokenKind::Identifier(maybe_tag_name) = maybe_tag.kind {
            Ok(Some(maybe_tag_name))
        } else {
            self.lexer.unget(maybe_tag);
            Ok(None)
        }
    }
    fn read_rectype_def(&mut self, is_struct: bool) -> ParseR<Type> {
        let tag = {
            let opt_tag = try!(self.read_rectype_tag());
            if opt_tag.is_some() {
                opt_tag.unwrap()
            } else {
                // if the rectype(struct|union) has no name(e.g. typedef struct { int a; } A),
                // generate a random name
                rand::thread_rng().gen_ascii_chars().take(8).collect()
            }
        };

        let fields = try!(self.read_rectype_fields());
        let cur_tags = self.tags.back_mut().unwrap();

        if fields.is_empty() {
            Ok(match cur_tags.entry(tag) {
                hash_map::Entry::Occupied(o) => o.get().clone(),
                hash_map::Entry::Vacant(v) => {
                    let new_struct = if is_struct {
                        Type::Struct(v.key().to_string(), Vec::new())
                    } else {
                        Type::Union(v.key().to_string(), Vec::new(), 0)
                    };
                    v.insert(new_struct).clone()
                }
            })
        } else {
            let new_rectype = if is_struct {
                Type::Struct(tag.to_string(), fields)
            } else {
                // if union
                let mut max_sz_ty_nth = 0;
                let mut max_sz = 0;
                for (i, field_decl) in (&fields).iter().enumerate() {
                    if let ASTKind::VariableDecl(ref ty, _, _, _) = field_decl.kind {
                        if ty.calc_size() > max_sz {
                            max_sz = ty.calc_size();
                            max_sz_ty_nth = i;
                        }
                    }
                }
                Type::Union(tag.to_string(), fields, max_sz_ty_nth)
            };
            Ok(match cur_tags.entry(tag) {
                hash_map::Entry::Occupied(o) => {
                    *o.into_mut() = new_rectype.clone();
                    new_rectype
                }
                hash_map::Entry::Vacant(v) => v.insert(new_rectype).clone(),
            })
        }
    }
    fn read_rectype_fields(&mut self) -> ParseR<Vec<AST>> {
        if !try!(self.lexer.skip_symbol(Symbol::OpeningBrace)) {
            return Ok(Vec::new());
        }

        let mut decls: Vec<AST> = Vec::new();
        loop {
            let peek = try!(self.lexer.peek());
            if !self.is_type(&peek) {
                break;
            }
            let (basety, _, _) = try!(self.read_type_spec());
            loop {
                let (ty, name, _) = try!(self.read_declarator(basety.clone()));
                if try!(self.lexer.skip_symbol(Symbol::Colon)) {
                    // TODO: for now, designated bitwidth ignore
                    try!(self.read_expr());
                }
                decls.push(AST::new(
                    ASTKind::VariableDecl(ty, name, StorageClass::Auto, None),
                    self.lexer.get_cur_pos(),
                ));
                if try!(self.lexer.skip_symbol(Symbol::Comma)) {
                    continue;
                } else {
                    expect_symbol_error!(self, Symbol::Semicolon, "expected ';'");
                }
                break;
            }
        }
        expect_symbol_error!(self, Symbol::ClosingBrace, "expected '}'");
        Ok(decls)
    }
    fn read_enum_def(&mut self) -> ParseR<Type> {
        let (tag, exist_tag) = {
            let opt_tag = try!(self.read_rectype_tag());
            if opt_tag.is_some() {
                (opt_tag.unwrap(), true)
            } else {
                ("".to_string(), false)
            }
        };
        if exist_tag {
            match self.tags.get(tag.as_str()) {
                Some(&Type::Enum) => {}
                None => {}
                _ => {
                    let peek = self.lexer.peek();
                    self.show_error_token(&try!(peek), "undefined enum");
                    return Err(Error::Something);
                }
            }
        }

        if !try!(self.lexer.skip_symbol(Symbol::OpeningBrace)) {
            if !exist_tag || !self.tags.contains(tag.as_str()) {
                let peek = self.lexer.peek();
                self.show_error_token(&try!(peek), "do not redefine enum");
                return Err(Error::Something);
            }
            return Ok(Type::Int(Sign::Signed));
        }

        if exist_tag {
            self.tags.add(tag, Type::Enum);
        }

        let mut val = 0;
        loop {
            if try!(self.lexer.skip_symbol(Symbol::ClosingBrace)) {
                break;
            }
            let name = ident_val!(try!(self.lexer.get()));
            if try!(self.lexer.skip_symbol(Symbol::Assign)) {
                val = match try!(self.read_assign()).eval_constexpr() {
                    Ok(val) => val,
                    Err(Error::Something) => {
                        let peek = try!(self.lexer.peek());
                        self.show_error_token(&peek, "enum initialize value must be constant");
                        0
                    }
                    Err(e) => return Err(e),
                };
            }
            let constval = AST::new(ASTKind::Int(val, Bits::Bits32), self.lexer.get_cur_pos());
            val += 1;
            self.env.add(name, constval);
            if try!(self.lexer.skip_symbol(Symbol::Comma)) {
                continue;
            }
            if try!(self.lexer.skip_symbol(Symbol::OpeningBrace)) {
                break;
            }
        }

        Ok(Type::Int(Sign::Signed))
    }

    pub fn read_expr(&mut self) -> ParseR<AST> {
        self.read_comma()
    }
    pub fn read_opt_expr(&mut self) -> ParseR<AST> {
        if try!(self.lexer.peek()).kind == TokenKind::Symbol(Symbol::Semicolon) {
            Ok(AST::new(
                ASTKind::Compound(Vec::new()),
                self.lexer.get_cur_pos(),
            ))
        } else {
            self.read_expr()
        }
    }
    fn read_comma(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_assign());
        while try!(self.lexer.skip_symbol(Symbol::Comma)) {
            let rhs = try!(self.read_assign());
            lhs = AST::new(
                ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Comma),
                self.lexer.get_cur_pos(),
            )
        }
        Ok(lhs)
    }
    fn read_assign(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_logor());
        if try!(self.lexer.skip_symbol(Symbol::Question)) {
            return self.read_ternary(lhs);
        }
        macro_rules! assign { ($lhs:expr, $rhs:expr, $pos:expr) => (
            AST::new(
                ASTKind::BinaryOp(Box::new($lhs), Box::new($rhs), node::CBinOps::Assign), $pos
            ) )
        }
        macro_rules! f { ($op:ident) => (
            lhs = assign!(
                lhs.clone(),
                AST::new(
                    ASTKind::BinaryOp(
                        Box::new(lhs),
                        Box::new(try!(self.read_assign())),
                        node::CBinOps::$op,
                    ),
                    self.lexer.get_cur_pos(),
                ),
                self.lexer.get_cur_pos()
            ); )
        }
        loop {
            let tok = try!(self.lexer.get());
            match tok.kind {
                TokenKind::Symbol(Symbol::Assign) => {
                    lhs = assign!(lhs, try!(self.read_assign()), self.lexer.get_cur_pos());
                }
                TokenKind::Symbol(Symbol::AssignAdd) => f!(Add),
                TokenKind::Symbol(Symbol::AssignSub) => f!(Sub),
                TokenKind::Symbol(Symbol::AssignMul) => f!(Mul),
                TokenKind::Symbol(Symbol::AssignDiv) => f!(Div),
                TokenKind::Symbol(Symbol::AssignMod) => f!(Rem),
                TokenKind::Symbol(Symbol::AssignShl) => f!(Shl),
                TokenKind::Symbol(Symbol::AssignShr) => f!(Shr),
                TokenKind::Symbol(Symbol::AssignAnd) => f!(And),
                TokenKind::Symbol(Symbol::AssignOr) => f!(Or),
                TokenKind::Symbol(Symbol::AssignXor) => f!(Xor),
                // TODO: implement more op
                _ => {
                    self.lexer.unget(tok);
                    break;
                }
            }
        }
        Ok(lhs)
    }
    fn read_ternary(&mut self, cond: AST) -> ParseR<AST> {
        let mut then_expr = try!(self.read_expr());
        expect_symbol_error!(self, Symbol::Colon, "expected ':'");
        let mut else_expr = try!(self.read_assign());
        let then_ty = try!(self.get_expr_returning_ty(&then_expr));
        let else_ty = try!(self.get_expr_returning_ty(&else_expr));
        if then_ty.is_arith_ty() && else_ty.is_arith_ty() {
            let ty = self.usual_binary_ty_cov(then_ty, else_ty);
            then_expr = self.cast_ast(&then_expr, &ty);
            else_expr = self.cast_ast(&else_expr, &ty);
        }
        Ok(AST::new(
            ASTKind::TernaryOp(Box::new(cond), Box::new(then_expr), Box::new(else_expr)),
            self.lexer.get_cur_pos(),
        ))
    }
    fn read_logor(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_logand());
        while try!(self.lexer.skip_symbol(Symbol::LOr)) {
            let rhs = try!(self.read_logand());
            lhs = AST::new(
                ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::LOr),
                self.lexer.get_cur_pos(),
            );
        }
        Ok(lhs)
    }
    fn read_logand(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_or());
        while try!(self.lexer.skip_symbol(Symbol::LAnd)) {
            let rhs = try!(self.read_or());
            lhs = AST::new(
                ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::LAnd),
                self.lexer.get_cur_pos(),
            );
        }
        Ok(lhs)
    }
    fn read_or(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_xor());
        while try!(self.lexer.skip_symbol(Symbol::Or)) {
            let rhs = try!(self.read_xor());
            lhs = AST::new(
                ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Or),
                self.lexer.get_cur_pos(),
            );
        }
        Ok(lhs)
    }
    fn read_xor(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_and());
        while try!(self.lexer.skip_symbol(Symbol::Xor)) {
            let rhs = try!(self.read_and());
            lhs = AST::new(
                ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Xor),
                self.lexer.get_cur_pos(),
            );
        }
        Ok(lhs)
    }
    fn read_and(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_eq_ne());
        while try!(self.lexer.skip_symbol(Symbol::Ampersand)) {
            let rhs = try!(self.read_eq_ne());
            lhs = AST::new(
                ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::And),
                self.lexer.get_cur_pos(),
            );
        }
        Ok(lhs)
    }
    fn read_eq_ne(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_relation());
        loop {
            if try!(self.lexer.skip_symbol(Symbol::Eq)) {
                let rhs = try!(self.read_relation());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Eq),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Ne)) {
                let rhs = try!(self.read_relation());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Ne),
                    self.lexer.get_cur_pos(),
                );
            } else {
                break;
            }
        }
        Ok(lhs)
    }
    fn read_relation(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_shl_shr());
        loop {
            if try!(self.lexer.skip_symbol(Symbol::Lt)) {
                let rhs = try!(self.read_shl_shr());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Lt),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Le)) {
                let rhs = try!(self.read_shl_shr());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Le),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Gt)) {
                let rhs = try!(self.read_shl_shr());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Gt),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Ge)) {
                let rhs = try!(self.read_shl_shr());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Ge),
                    self.lexer.get_cur_pos(),
                );
            } else {
                break;
            }
        }
        Ok(lhs)
    }
    fn read_shl_shr(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_add_sub());
        loop {
            if try!(self.lexer.skip_symbol(Symbol::Shl)) {
                let rhs = try!(self.read_add_sub());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Shl),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Shr)) {
                let rhs = try!(self.read_add_sub());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Shr),
                    self.lexer.get_cur_pos(),
                );
            } else {
                break;
            }
        }
        Ok(lhs)
    }
    fn read_add_sub(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_mul_div_rem());
        loop {
            if try!(self.lexer.skip_symbol(Symbol::Add)) {
                let rhs = try!(self.read_mul_div_rem());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Add),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Sub)) {
                let rhs = try!(self.read_mul_div_rem());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Sub),
                    self.lexer.get_cur_pos(),
                );
            } else {
                break;
            }
        }
        Ok(lhs)
    }
    fn read_mul_div_rem(&mut self) -> ParseR<AST> {
        let mut lhs = try!(self.read_cast());
        loop {
            if try!(self.lexer.skip_symbol(Symbol::Asterisk)) {
                let rhs = try!(self.read_cast());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Mul),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Div)) {
                let rhs = try!(self.read_cast());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Div),
                    self.lexer.get_cur_pos(),
                );
            } else if try!(self.lexer.skip_symbol(Symbol::Mod)) {
                let rhs = try!(self.read_cast());
                lhs = AST::new(
                    ASTKind::BinaryOp(Box::new(lhs), Box::new(rhs), node::CBinOps::Rem),
                    self.lexer.get_cur_pos(),
                );
            } else {
                break;
            }
        }
        Ok(lhs)
    }
    fn read_cast(&mut self) -> ParseR<AST> {
        let tok = try!(self.lexer.get());
        let peek = try!(self.lexer.peek());
        if tok.kind == TokenKind::Symbol(Symbol::OpeningParen) && self.is_type(&peek) {
            let basety = try!(self.read_type_spec()).0;
            let ty = try!(self.read_declarator(basety)).0;
            expect_symbol_error!(self, Symbol::ClosingParen, "expected ')'");
            return Ok(AST::new(
                ASTKind::TypeCast(Box::new(try!(self.read_cast())), ty),
                self.lexer.get_cur_pos(),
            ));
        } else {
            self.lexer.unget(tok);
        }
        self.read_unary()
    }
    fn read_unary(&mut self) -> ParseR<AST> {
        let tok = try!(self.lexer.get());
        match tok.kind {
            TokenKind::Symbol(Symbol::Not) => {
                return Ok(AST::new(
                    ASTKind::UnaryOp(Box::new(try!(self.read_cast())), node::CUnaryOps::LNot),
                    self.lexer.get_cur_pos(),
                ))
            }
            TokenKind::Symbol(Symbol::BitwiseNot) => {
                return Ok(AST::new(
                    ASTKind::UnaryOp(Box::new(try!(self.read_cast())), node::CUnaryOps::BNot),
                    self.lexer.get_cur_pos(),
                ))
            }
            TokenKind::Symbol(Symbol::Add) => return self.read_cast(),
            TokenKind::Symbol(Symbol::Sub) => {
                return Ok(AST::new(
                    ASTKind::UnaryOp(Box::new(try!(self.read_cast())), node::CUnaryOps::Minus),
                    self.lexer.get_cur_pos(),
                ))
            }
            TokenKind::Symbol(Symbol::Inc) => {
                let pos = self.lexer.get_cur_pos();
                let var = try!(self.read_cast());
                return Ok(AST::new(
                    ASTKind::BinaryOp(
                        Box::new(var.clone()),
                        Box::new(AST::new(
                            ASTKind::BinaryOp(
                                Box::new(var),
                                Box::new(AST::new(ASTKind::Int(1, Bits::Bits32), pos.clone())),
                                node::CBinOps::Add,
                            ),
                            pos.clone(),
                        )),
                        node::CBinOps::Assign,
                    ),
                    pos,
                ));
            }
            TokenKind::Symbol(Symbol::Dec) => {
                let pos = self.lexer.get_cur_pos();
                let var = try!(self.read_cast());
                return Ok(AST::new(
                    ASTKind::BinaryOp(
                        Box::new(var.clone()),
                        Box::new(AST::new(
                            ASTKind::BinaryOp(
                                Box::new(var),
                                Box::new(AST::new(ASTKind::Int(1, Bits::Bits32), pos.clone())),
                                node::CBinOps::Sub,
                            ),
                            pos.clone(),
                        )),
                        node::CBinOps::Assign,
                    ),
                    pos,
                ));
            }
            TokenKind::Symbol(Symbol::Asterisk) => {
                return Ok(AST::new(
                    ASTKind::UnaryOp(Box::new(try!(self.read_cast())), node::CUnaryOps::Deref),
                    self.lexer.get_cur_pos(),
                ))
            }
            TokenKind::Symbol(Symbol::Ampersand) => {
                return Ok(AST::new(
                    ASTKind::UnaryOp(Box::new(try!(self.read_cast())), node::CUnaryOps::Addr),
                    self.lexer.get_cur_pos(),
                ))
            }
            TokenKind::Symbol(Symbol::Sizeof) => {
                // TODO: must fix this sloppy implementation
                return self.read_sizeof();
            }
            _ => {}
        }
        self.lexer.unget(tok);
        self.read_postfix()
    }
    fn read_sizeof(&mut self) -> ParseR<AST> {
        let tok = try!(self.lexer.get());
        let peek = try!(self.lexer.peek());
        if matches!(tok.kind, TokenKind::Symbol(Symbol::OpeningParen)) && self.is_type(&peek) {
            let (basety, _, _) = try!(self.read_type_spec());
            let (ty, _, _) = try!(self.read_declarator(basety));
            try!(self.lexer.skip_symbol(Symbol::ClosingParen));
            return Ok(AST::new(
                ASTKind::Int(ty.calc_size() as i64, Bits::Bits32),
                self.lexer.get_cur_pos(),
            ));
        }
        self.lexer.unget(tok);
        let expr = try!(self.read_unary());
        Ok(AST::new(
            ASTKind::Int(try!(self.calc_sizeof(&expr)) as i64, Bits::Bits32),
            self.lexer.get_cur_pos(),
        ))
    }
    fn read_postfix(&mut self) -> ParseR<AST> {
        let mut ast = try!(self.read_primary());
        loop {
            if try!(self.lexer.skip_symbol(Symbol::OpeningParen)) {
                ast = try!(self.read_func_call(ast));
                continue;
            }
            if try!(self.lexer.skip_symbol(Symbol::OpeningBoxBracket)) {
                ast = AST::new(
                    ASTKind::Load(Box::new(try!(self.read_index(ast)))),
                    self.lexer.get_cur_pos(),
                );
                continue;
            }
            if try!(self.lexer.skip_symbol(Symbol::Point)) {
                ast = AST::new(
                    ASTKind::Load(Box::new(try!(self.read_field(ast)))),
                    self.lexer.get_cur_pos(),
                );
                continue;
            }
            if try!(self.lexer.skip_symbol(Symbol::Arrow)) {
                let pos = self.lexer.get_cur_pos();
                let field = try!(self.read_field(AST::new(
                    ASTKind::UnaryOp(Box::new(ast), node::CUnaryOps::Deref),
                    pos.clone()
                )));
                ast = AST::new(ASTKind::Load(Box::new(field)), pos);
                continue;
            }
            if try!(self.lexer.skip_symbol(Symbol::Inc)) {
                return Ok(AST::new(
                    ASTKind::UnaryOp(Box::new(ast), node::CUnaryOps::Inc),
                    self.lexer.get_cur_pos(),
                ));
            }
            if try!(self.lexer.skip_symbol(Symbol::Dec)) {
                return Ok(AST::new(
                    ASTKind::UnaryOp(Box::new(ast), node::CUnaryOps::Dec),
                    self.lexer.get_cur_pos(),
                ));
            }
            break;
        }
        Ok(ast)
    }
    fn read_func_call(&mut self, f: AST) -> ParseR<AST> {
        let pos = self.lexer.get_cur_pos();
        let mut args = Vec::new();
        if !try!(self.lexer.skip_symbol(Symbol::ClosingParen)) {
            loop {
                match self.read_assign() {
                    Ok(arg) => args.push(arg),
                    Err(_) => {}
                }

                if try!(self.lexer.skip_symbol(Symbol::ClosingParen)) {
                    break;
                }
                if !try!(self.lexer.skip_symbol(Symbol::Comma)) {
                    let peek = self.lexer.peek();
                    self.show_error_token(&try!(peek), "expected ','");
                    self.skip_until(Symbol::ClosingParen);
                    return Err(Error::Something);
                }
            }
        }

        Ok(AST::new(ASTKind::FuncCall(Box::new(f), args), pos))
    }
    fn read_index(&mut self, ast: AST) -> ParseR<AST> {
        let idx = try!(self.read_expr());
        expect_symbol_error!(self, Symbol::ClosingBoxBracket, "expected ']'");
        Ok(AST::new(
            ASTKind::BinaryOp(Box::new(ast), Box::new(idx), node::CBinOps::Add),
            self.lexer.get_cur_pos(),
        ))
    }

    fn read_field(&mut self, ast: AST) -> ParseR<AST> {
        let field = try!(self.lexer.get());
        if !matches!(field.kind, TokenKind::Identifier(_)) {
            let peek = self.lexer.peek();
            self.show_error_token(&try!(peek), "expected field name");
            return Err(Error::Something);
        }

        let field_name = ident_val!(field);
        Ok(AST::new(
            ASTKind::StructRef(Box::new(ast), field_name),
            self.lexer.get_cur_pos(),
        ))
    }

    fn read_primary(&mut self) -> ParseR<AST> {
        let tok = match self.lexer.get() {
            Ok(tok) => tok,
            Err(_) => {
                let peek = self.lexer.peek();
                self.show_error_token(
                    &try!(peek),
                    "expected primary(number, string...), but reach EOF",
                );
                return Err(Error::Something);
            }
        };

        match tok.kind.clone() {
            TokenKind::IntNumber(n, bits) => {
                Ok(AST::new(ASTKind::Int(n, bits), self.lexer.get_cur_pos()))
            }
            TokenKind::FloatNumber(f) => Ok(AST::new(ASTKind::Float(f), self.lexer.get_cur_pos())),
            TokenKind::Identifier(ident) => {
                if let Some(ast) = self.env.get(ident.as_str()) {
                    return match ast.kind {
                        ASTKind::Variable(_, _) => Ok(AST::new(
                            ASTKind::Load(Box::new((*ast).clone())),
                            self.lexer.get_cur_pos(),
                        )),
                        _ => Ok((*ast).clone()),
                    };
                }
                self.show_error_token(
                    &tok,
                    format!("not found the variable or function '{}'", ident).as_str(),
                );
                Err(Error::Something)
            }
            TokenKind::String(s) => Ok(AST::new(ASTKind::String(s), self.lexer.get_cur_pos())),
            TokenKind::Char(ch) => Ok(AST::new(ASTKind::Char(ch as i32), self.lexer.get_cur_pos())),
            TokenKind::Symbol(sym) => match sym {
                Symbol::OpeningParen => {
                    let expr = self.read_expr();
                    if !try!(self.lexer.skip_symbol(Symbol::ClosingParen)) {
                        self.show_error_token(&tok, "expected ')'");
                    }
                    expr
                }
                _ => {
                    self.show_error_token(
                        &tok,
                        format!("expected primary section, but got {:?}", tok.kind).as_str(),
                    );
                    Err(Error::Something)
                }
            },
            _ => {
                self.show_error_token(
                    &tok,
                    format!("read_primary unknown token {:?}", tok.kind).as_str(),
                );
                Err(Error::Something)
            }
        }
    }

    fn usual_binary_ty_cov(&mut self, lhs: Type, rhs: Type) -> Type {
        if lhs.priority() < rhs.priority() {
            rhs
        } else {
            lhs
        }
    }
    fn get_binary_expr_ty(&mut self, lhs: &AST, rhs: &AST, op: &node::CBinOps) -> ParseR<Type> {
        fn cast(ty: Type) -> Type {
            match ty {
                Type::Array(elem_ty, _) => Type::Ptr(elem_ty),
                Type::Func(_, _, _) => Type::Ptr(Box::new(ty)),
                _ => ty,
            }
        }
        let lhs_ty = cast(try!(self.get_expr_returning_ty(lhs)));
        let rhs_ty = cast(try!(self.get_expr_returning_ty(rhs)));
        if matches!(lhs_ty, Type::Ptr(_)) && matches!(rhs_ty, Type::Ptr(_)) {
            if matches!(op, &node::CBinOps::Sub) {
                return Ok(Type::Long(Sign::Signed));
            }
            return Ok(Type::Int(Sign::Signed));
        }
        if matches!(lhs_ty, Type::Ptr(_)) {
            return Ok(lhs_ty);
        }
        if matches!(rhs_ty, Type::Ptr(_)) {
            return Ok(rhs_ty);
        }
        return Ok(self.usual_binary_ty_cov(lhs_ty, rhs_ty));
    }
    fn get_expr_returning_ty(&mut self, ast: &AST) -> ParseR<Type> {
        let size = match ast.kind {
            ASTKind::Int(_, Bits::Bits32) => Type::Int(Sign::Signed),
            ASTKind::Int(_, Bits::Bits64) => Type::Long(Sign::Signed),
            ASTKind::Float(_) => Type::Double,
            ASTKind::Char(_) => Type::Char(Sign::Signed),
            ASTKind::String(ref s) => {
                Type::Array(Box::new(Type::Char(Sign::Signed)), s.len() as i32 + 1)
            }
            ASTKind::Load(ref v) => {
                (*try!(self.get_expr_returning_ty(&*v)).get_elem_ty().unwrap()).clone()
            }
            ASTKind::Variable(ref ty, _) => Type::Ptr(Box::new((*ty).clone())),
            ASTKind::UnaryOp(_, node::CUnaryOps::LNot) => Type::Int(Sign::Signed),
            ASTKind::UnaryOp(ref expr, node::CUnaryOps::Minus)
            | ASTKind::UnaryOp(ref expr, node::CUnaryOps::Inc)
            | ASTKind::UnaryOp(ref expr, node::CUnaryOps::Dec)
            | ASTKind::UnaryOp(ref expr, node::CUnaryOps::BNot) => {
                try!(self.get_expr_returning_ty(&*expr))
            }
            ASTKind::UnaryOp(ref expr, node::CUnaryOps::Deref) => (*try!(
                self.get_expr_returning_ty(&*expr)
            ).get_elem_ty()
                .unwrap())
                .clone(),
            ASTKind::UnaryOp(ref expr, node::CUnaryOps::Addr) => {
                Type::Ptr(Box::new(try!(self.get_expr_returning_ty(&*expr))))
            }
            ASTKind::StructRef(ref expr, ref name) => {
                let ty = try!(self.get_expr_returning_ty(expr));
                Type::Ptr(Box::new((*ty.get_field_ty(name.as_str()).unwrap()).clone()))
            }
            ASTKind::TypeCast(_, ref ty) => ty.clone(),
            ASTKind::BinaryOp(ref lhs, ref rhs, ref op) => {
                try!(self.get_binary_expr_ty(&*lhs, &*rhs, &*op))
            }
            ASTKind::TernaryOp(_, ref then, _) => try!(self.get_expr_returning_ty(&*then)),
            ASTKind::FuncCall(ref func, _) => {
                let func_ty = try!(self.get_expr_returning_ty(func));
                (*func_ty.get_return_ty().unwrap()).clone()
            }
            _ => panic!(format!("unsupported: {:?}", ast.kind)),
        };
        Ok(size)
    }
    fn calc_sizeof(&mut self, ast: &AST) -> ParseR<usize> {
        let ty = try!(self.get_expr_returning_ty(ast));
        Ok(ty.calc_size())
    }

    fn cast_ast(&mut self, expr: &AST, ty: &Type) -> AST {
        AST::new(
            ASTKind::TypeCast(Box::new(expr.clone()), ty.clone()),
            expr.pos.clone(),
        )
    }
}
