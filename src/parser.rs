use lexer::{Keyword, Lexer, Pos, Symbol, Token, TokenKind};
use codegen;
use node::{ASTKind, Bits, AST};
use node;
use types::{Sign, StorageClass, Type};

use std::{str, u32};
use std::rc::Rc;
use std::io::{stderr, Write};
use std::collections::{hash_map, HashMap, HashSet, VecDeque};

use CODEGEN;

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

use std::sync::Mutex;
use std::marker::Send;

unsafe impl Send for Type {}

pub static mut ERROR_COUNTS: usize = 0;

lazy_static! {
    static ref ENV: Mutex<VecDeque<HashMap<String, AST>>> = {
        let mut v = VecDeque::new();
        v.push_back(HashMap::new());
        Mutex::new(v)
    };
    static ref TAGS: Mutex<VecDeque<HashMap<String, Type>>> = {
        let mut v = VecDeque::new();
        v.push_back(HashMap::new());
        Mutex::new(v)
    };
    static ref CONSTEXPR_FUNC_MAP: Mutex<HashSet<String>> = {
        Mutex::new(HashSet::new())
    };
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
    ($lexer:expr, $sym:expr, $msg:expr) => {{
        if !try!($lexer.skip_symbol($sym)) {
            let peek = $lexer.peek();
            show_error_token($lexer, &try!(peek), $msg);
        }
    }}
}

fn show_error(lexer: &mut Lexer, msg: &str) {
    unsafe {
        ERROR_COUNTS += 1;
    }
    writeln!(
        &mut stderr(),
        "{}: {} {}: {}",
        lexer.get_filename(),
        Colour::Red.bold().paint("error:"),
        lexer.get_cur_line(),
        msg
    ).unwrap();
}
fn show_error_token(lexer: &mut Lexer, token: &Token, msg: &str) {
    unsafe {
        ERROR_COUNTS += 1;
    }
    writeln!(
        &mut stderr(),
        "{}: {} {}: {}",
        lexer.get_filename(),
        Colour::Red.bold().paint("error:"),
        token.pos.line,
        msg
    ).unwrap();
    writeln!(
        &mut stderr(),
        "{}",
        lexer.get_surrounding_code_with_err_point(token.pos.pos,)
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
    run(&mut lexer, &mut nodes);
    nodes
}
pub fn run(lexer: &mut Lexer, node: &mut Vec<AST>) {
    while matches!(read_toplevel(lexer, node), Ok(_)) {}
    show_total_errors();
}
pub fn run_as_expr(lexer: &mut Lexer) -> ParseR<AST> {
    read_expr(lexer)
}
pub fn show_total_errors() {
    unsafe {
        if ERROR_COUNTS > 0 {
            println!(
                "{} error{} generated.",
                ERROR_COUNTS,
                if ERROR_COUNTS > 1 { "s" } else { "" }
            );
            ::std::process::exit(-1);
        }
    }
}
pub fn read_toplevel(lexer: &mut Lexer, ast: &mut Vec<AST>) -> ParseR<()> {
    // TODO: refine
    if try!(is_function_def(lexer)) {
        match read_func_def(lexer) {
            Ok(ok) => ast.push(ok),
            Err(Error::EOF) => show_error(lexer, "expected a token, but reached EOF"),
            Err(e) => return Err(e),
        }
    } else {
        match read_decl(lexer, ast) {
            Err(Error::EOF) => show_error(lexer, "expected a token, but reached EOF"),
            Err(e) => return Err(e),
            _ => {}
        }
    }
    Ok(())
}
fn read_func_def(lexer: &mut Lexer) -> ParseR<AST> {
    let localenv = (*ENV.lock().unwrap().back().unwrap()).clone();
    let localtags = (*TAGS.lock().unwrap().back().unwrap()).clone();
    ENV.lock().unwrap().push_back(localenv);
    TAGS.lock().unwrap().push_back(localtags);

    let (ret_ty, _, qualifiers) = try!(read_type_spec(lexer));
    let (functy, name, param_names) = try!(read_declarator(lexer, ret_ty));

    // TODO: do really have to support constexpr?
    // if qualifiers.q_constexpr {
    //     CONSTEXPR_FUNC_MAP.lock().unwrap().insert(name.clone());
    // }
    {
        let mut env = ENV.lock().unwrap();
        // [0] is global env, [1] is local env. so we have to insert to both.
        env[0].insert(
            name.clone(),
            AST::new(
                ASTKind::Variable(functy.clone(), name.clone()),
                Pos::new(0, 0),
            ),
        );
        env[1].insert(
            name.clone(),
            AST::new(
                ASTKind::Variable(functy.clone(), name.clone()),
                Pos::new(0, 0),
            ),
        );
        env[1].insert(
            "__func__".to_string(),
            AST::new(ASTKind::String(name.clone()), Pos::new(0, 0)),
        );
    }

    expect_symbol_error!(lexer, Symbol::OpeningBrace, "expected '('");
    let body = try!(read_func_body(lexer, &functy));

    ENV.lock().unwrap().pop_back();
    TAGS.lock().unwrap().pop_back();

    Ok(AST::new(
        ASTKind::FuncDef(
            functy,
            if param_names.is_none() {
                Vec::new()
            } else {
                param_names.unwrap()
            },
            name,
            Rc::new(body),
        ),
        Pos::new(0, 0),
    ))
}
fn read_func_body(lexer: &mut Lexer, _functy: &Type) -> ParseR<AST> {
    read_compound_stmt(lexer)
}
fn read_compound_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    let mut stmts: Vec<AST> = Vec::new();
    loop {
        if try!(lexer.skip_symbol(Symbol::ClosingBrace).or_else(|eof| {
            show_error(lexer, "expected '}'");
            Err(eof)
        })) {
            break;
        }

        let peek_tok = try!(lexer.peek());
        if is_type(&peek_tok) {
            // variable declaration
            try!(read_decl(lexer, &mut stmts));
        } else {
            match read_stmt(lexer) {
                Ok(stmt) => stmts.push(stmt),
                Err(_) => {}
            }
        }
    }
    Ok(AST::new(ASTKind::Block(stmts), Pos::new(0, 0)))
}
fn read_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    let tok = try!(lexer.get());
    if let &TokenKind::Keyword(ref keyw) = &tok.kind {
        match *keyw {
            Keyword::If => return read_if_stmt(lexer),
            Keyword::For => return read_for_stmt(lexer),
            Keyword::While => return read_while_stmt(lexer),
            Keyword::Do => return read_do_while_stmt(lexer),
            Keyword::Switch => return read_switch_stmt(lexer),
            Keyword::Case => return read_case_label(lexer),
            Keyword::Default => return read_default_label(lexer),
            Keyword::Goto => return read_goto_stmt(lexer),
            Keyword::Continue => return read_continue_stmt(lexer),
            Keyword::Break => return read_break_stmt(lexer),
            Keyword::Return => return read_return_stmt(lexer),
            _ => {}
        }
    } else if let &TokenKind::Symbol(Symbol::OpeningBrace) = &tok.kind {
        return read_compound_stmt(lexer);
    }

    if matches!(tok.kind, TokenKind::Identifier(_))
        && try!(lexer.peek_symbol_token_is(Symbol::Colon))
    {
        return read_label(lexer, tok);
    }

    lexer.unget(tok);
    let expr = read_opt_expr(lexer);
    expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
    expr
}
fn read_if_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    expect_symbol_error!(lexer, Symbol::OpeningParen, "expected '('");
    let cond = try!(read_expr(lexer));
    expect_symbol_error!(lexer, Symbol::ClosingParen, "expected ')'");
    let then_stmt = Rc::new(try!(read_stmt(lexer)));
    let else_stmt = if try!(lexer.skip_keyword(Keyword::Else)) {
        Rc::new(try!(read_stmt(lexer)))
    } else {
        Rc::new(AST::new(ASTKind::Block(Vec::new()), Pos::new(0, 0)))
    };
    Ok(AST::new(
        ASTKind::If(Rc::new(cond), then_stmt, else_stmt),
        Pos::new(0, 0),
    ))
}
fn read_for_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    expect_symbol_error!(lexer, Symbol::OpeningParen, "expected '('");
    let init = try!(read_opt_decl_or_stmt(lexer));
    // TODO: read_expr should return Option<AST>.
    //       when cur tok is ';', returns None.
    let cond = try!(read_opt_expr(lexer));
    expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
    let step = if try!(lexer.peek_symbol_token_is(Symbol::ClosingParen)) {
        AST::new(ASTKind::Compound(Vec::new()), lexer.get_cur_pos())
    } else {
        try!(read_opt_expr(lexer))
    };
    expect_symbol_error!(lexer, Symbol::ClosingParen, "expected ')'");
    let body = try!(read_stmt(lexer));
    Ok(AST::new(
        ASTKind::For(Rc::new(init), Rc::new(cond), Rc::new(step), Rc::new(body)),
        Pos::new(0, 0),
    ))
}
fn read_while_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    expect_symbol_error!(lexer, Symbol::OpeningParen, "expected '('");
    let cond = try!(read_expr(lexer));
    expect_symbol_error!(lexer, Symbol::ClosingParen, "expected ')'");
    let body = try!(read_stmt(lexer));
    Ok(AST::new(
        ASTKind::While(Rc::new(cond), Rc::new(body)),
        Pos::new(0, 0),
    ))
}
fn read_do_while_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    let body = try!(read_stmt(lexer));
    if !try!(lexer.skip_keyword(Keyword::While)) {
        let peek = lexer.peek();
        show_error_token(lexer, &try!(peek), "expected 'while'");
    }
    expect_symbol_error!(lexer, Symbol::OpeningParen, "expected '('");
    let cond = try!(read_expr(lexer));
    expect_symbol_error!(lexer, Symbol::ClosingParen, "expected ')'");
    expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
    Ok(AST::new(
        ASTKind::DoWhile(Rc::new(cond), Rc::new(body)),
        Pos::new(0, 0),
    ))
}
fn read_switch_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    expect_symbol_error!(lexer, Symbol::OpeningParen, "expected '('");
    let cond = try!(read_expr(lexer));
    expect_symbol_error!(lexer, Symbol::ClosingParen, "expected ')'");
    let body = Rc::new(try!(read_stmt(lexer)));
    Ok(AST::new(
        ASTKind::Switch(Rc::new(cond), body),
        Pos::new(0, 0),
    ))
}
fn read_case_label(lexer: &mut Lexer) -> ParseR<AST> {
    let expr = try!(read_expr(lexer));
    expect_symbol_error!(lexer, Symbol::Colon, "expected ':'");
    Ok(AST::new(ASTKind::Case(Rc::new(expr)), Pos::new(0, 0)))
}
fn read_default_label(lexer: &mut Lexer) -> ParseR<AST> {
    expect_symbol_error!(lexer, Symbol::Colon, "expected ':'");
    Ok(AST::new(ASTKind::DefaultL, Pos::new(0, 0)))
}
fn read_goto_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    let pos = lexer.get_cur_pos();
    let label_name = ident_val!(try!(lexer.get()));
    expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
    Ok(AST::new(ASTKind::Goto(label_name), pos))
}
fn read_label(lexer: &mut Lexer, tok: Token) -> ParseR<AST> {
    let pos = lexer.get_cur_pos();
    let label_name = ident_val!(tok);
    expect_symbol_error!(lexer, Symbol::Colon, "expected ':'");
    Ok(AST::new(ASTKind::Label(label_name), pos))
}
fn read_continue_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    let pos = lexer.get_cur_pos();
    expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
    Ok(AST::new(ASTKind::Continue, pos))
}
fn read_break_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    let pos = lexer.get_cur_pos();
    expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
    Ok(AST::new(ASTKind::Break, pos))
}
fn read_return_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    let pos = lexer.get_cur_pos();
    if try!(lexer.skip_symbol(Symbol::Semicolon)) {
        Ok(AST::new(ASTKind::Return(None), pos))
    } else {
        let retval = Some(Rc::new(try!(read_expr(lexer))));
        expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
        Ok(AST::new(ASTKind::Return(retval), pos))
    }
}
fn is_function_def(lexer: &mut Lexer) -> ParseR<bool> {
    let mut buf = Vec::new();
    let mut is_funcdef = false;

    loop {
        let mut tok = try!(lexer.get());
        buf.push(tok.clone());

        if tok.kind == TokenKind::Symbol(Symbol::Semicolon) {
            break;
        }

        if is_type(&tok) {
            continue;
        }

        if tok.kind == TokenKind::Symbol(Symbol::OpeningParen) {
            try!(skip_parens(lexer, &tok, &mut buf));
            continue;
        }

        if !matches!(tok.kind, TokenKind::Identifier(_)) {
            continue;
        }

        if try!(lexer.peek()).kind != TokenKind::Symbol(Symbol::OpeningParen) {
            continue;
        }

        let opening_paren = try!(lexer.get());
        buf.push(opening_paren.clone());
        try!(skip_parens(lexer, &opening_paren, &mut buf));

        tok = try!(lexer.peek());
        is_funcdef = tok.kind == TokenKind::Symbol(Symbol::OpeningBrace);
        break;
    }

    lexer.unget_all(&buf);
    Ok(is_funcdef)
}
fn skip_parens(lexer: &mut Lexer, opening_paren: &Token, buf: &mut Vec<Token>) -> ParseR<()> {
    loop {
        let tok = try!(lexer.get().or_else(|_| {
            show_error_token(lexer, &opening_paren, "expected ')', but reach EOF");
            return Err(Error::Something);
        }));
        buf.push(tok.clone());

        match tok.kind {
            TokenKind::Symbol(Symbol::OpeningParen) => try!(skip_parens(lexer, &tok, buf)),
            TokenKind::Symbol(Symbol::ClosingParen) => break,
            _ => {}
        };
    }
    Ok(())
}
fn skip_until(lexer: &mut Lexer, sym: Symbol) {
    let ts = TokenKind::Symbol(sym);
    while match lexer.get() {
        Ok(tok) => tok.kind != ts,
        Err(_) => false,
    } {}
}

fn get_typedef(lexer: &mut Lexer, name: &str) -> ParseR<Option<Type>> {
    match ENV.lock().unwrap().back().unwrap().get(name) {
        Some(ast) => match ast.kind {
            ASTKind::Typedef(ref from, ref _to) => {
                let ty = match from {
                    &Type::Struct(ref name, ref fields) | &Type::Union(ref name, ref fields, _) => {
                        if fields.is_empty() {
                            TAGS.lock()
                                .unwrap()
                                .back()
                                .unwrap()
                                .get(name.as_str())
                                .unwrap()
                                .clone()
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
fn is_type(token: &Token) -> bool {
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
        match ENV.lock().unwrap().back().unwrap().get(ident.as_str()) {
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
fn is_string(ty: &Type) -> bool {
    if let &Type::Array(ref elem_ty, _) = ty {
        if matches!(**elem_ty, Type::Char(Sign::Signed)) {
            return true;
        }
    }
    false
}
fn read_decl_init(lexer: &mut Lexer, ty: &mut Type) -> ParseR<AST> {
    // TODO: implement for like 'int a[] = {...}, char *s="str";'
    if try!(lexer.peek_symbol_token_is(Symbol::OpeningBrace)) {
        return read_initializer_list(lexer, ty);
    } else if is_string(ty) {
        let tok = try!(lexer.get());
        if let TokenKind::String(s) = tok.kind {
            return read_string_initializer(lexer, ty, s);
        }
        lexer.unget(tok);
    }
    read_assign(lexer)
}
fn read_initializer_elem(lexer: &mut Lexer, ty: &mut Type) -> ParseR<AST> {
    if match *ty {
        Type::Array(_, _) | Type::Struct(_, _) | Type::Union(_, _, _) => true,
        _ => false,
    } {
        read_initializer_list(lexer, ty)
    } else if try!(lexer.peek_symbol_token_is(Symbol::OpeningBrace)) {
        let elem = read_initializer_elem(lexer, ty);
        expect_symbol_error!(lexer, Symbol::ClosingBrace, "expected '}'");
        elem
    } else {
        read_assign(lexer)
    }
}
fn read_initializer_list(lexer: &mut Lexer, ty: &mut Type) -> ParseR<AST> {
    if is_string(ty) {
        let tok = try!(lexer.get());
        if let TokenKind::String(s) = tok.kind {
            return read_string_initializer(lexer, ty, s);
        }
        lexer.unget(tok);
    }
    match ty {
        &mut Type::Array(_, _) => read_array_initializer(lexer, ty),
        &mut Type::Struct(_, _) | &mut Type::Union(_, _, _) => read_struct_initializer(lexer, ty),
        _ => read_assign(lexer),
    }
}
fn read_string_initializer(lexer: &mut Lexer, ty: &mut Type, string: String) -> ParseR<AST> {
    let mut char_ary = Vec::new();
    for c in string.chars() {
        char_ary.push(AST::new(ASTKind::Char(c as i32), Pos::new(0, 0)));
    }
    if let &mut Type::Array(_, ref mut len) = ty {
        *len = char_ary.len() as i32 + 1;
    } else {
        panic!()
    }
    Ok(AST::new(ASTKind::ConstArray(char_ary), lexer.get_cur_pos()))
}
fn read_array_initializer(lexer: &mut Lexer, ty: &mut Type) -> ParseR<AST> {
    let has_brace = try!(lexer.skip_symbol(Symbol::OpeningBrace));

    if let &mut Type::Array(ref elem_ty, ref mut len) = ty {
        let is_flexible = *len < 0;
        let mut elems = Vec::new();
        let mut elem_ty = (**elem_ty).clone();
        loop {
            let tok = try!(lexer.get());
            if let TokenKind::Symbol(Symbol::ClosingBrace) = tok.kind {
                if !has_brace {
                    lexer.unget(tok);
                }
                break;
            }
            lexer.unget(tok);
            let elem = try!(read_initializer_elem(lexer, &mut elem_ty));
            elems.push(elem);
            try!(lexer.skip_symbol(Symbol::Comma));
        }
        if is_flexible {
            *len = elems.len() as i32;
        }
        Ok(AST::new(ASTKind::ConstArray(elems), lexer.get_cur_pos()))
    } else {
        // maybe, this block never reach though.
        show_error(lexer, "initializer of array must be array");
        Err(Error::Something)
    }
}
fn read_struct_initializer(lexer: &mut Lexer, ty: &mut Type) -> ParseR<AST> {
    let tok = try!(lexer.get());
    let has_brace = tok.kind == TokenKind::Symbol(Symbol::OpeningBrace);

    let mut fields_types = if let Some(fields_types) = ty.get_all_fields_types() {
        fields_types
    } else {
        show_error_token(lexer, &tok, "initializer of struct must be array");
        return Err(Error::Something);
    };

    let mut elems = Vec::new();
    let mut field_type = fields_types.iter_mut();
    loop {
        let tok = try!(lexer.get());
        if let TokenKind::Symbol(Symbol::ClosingBrace) = tok.kind {
            if !has_brace {
                lexer.unget(tok);
            }
            break;
        }
        lexer.unget(tok);
        let elem = try!(read_initializer_elem(
            lexer,
            &mut field_type.next().unwrap().clone()
        ));
        elems.push(elem);
        try!(lexer.skip_symbol(Symbol::Comma));
    }
    Ok(AST::new(ASTKind::ConstStruct(elems), lexer.get_cur_pos()))
}
fn skip_type_qualifiers(lexer: &mut Lexer) -> ParseR<()> {
    while try!(lexer.skip_keyword(Keyword::Const)) || try!(lexer.skip_keyword(Keyword::Volatile))
        || try!(lexer.skip_keyword(Keyword::Restrict))
    {}
    Ok(())
}
fn read_decl(lexer: &mut Lexer, ast: &mut Vec<AST>) -> ParseR<()> {
    let (basety, sclass, qualifiers) = try!(read_type_spec(lexer));
    let is_typedef = sclass == StorageClass::Typedef;

    if try!(lexer.skip_symbol(Symbol::Semicolon)) {
        return Ok(());
    }

    loop {
        let (mut ty, name, _) = try!(read_declarator(lexer, basety.clone())); // XXX

        if (qualifiers.q_constexpr || qualifiers.q_const) && try!(lexer.skip_symbol(Symbol::Assign))
        {
            let init = try!(read_decl_init(lexer, &mut ty));
            ENV.lock()
                .unwrap()
                .back_mut()
                .unwrap()
                .insert(name.clone(), init);
        } else {
            if is_typedef {
                let typedef = AST::new(ASTKind::Typedef(ty, name.to_string()), lexer.get_cur_pos());
                ENV.lock()
                    .unwrap()
                    .back_mut()
                    .unwrap()
                    .insert(name, typedef);
                return Ok(());
            }

            let init = if try!(lexer.skip_symbol(Symbol::Assign)) {
                Some(Rc::new(try!(read_decl_init(lexer, &mut ty))))
            } else {
                None
            };
            ENV.lock().unwrap().back_mut().unwrap().insert(
                name.clone(),
                AST::new(ASTKind::Variable(ty.clone(), name.clone()), Pos::new(0, 0)),
            );
            ast.push(AST::new(
                ASTKind::VariableDecl(ty, name, sclass.clone(), init),
                lexer.get_cur_pos(),
            ));
        }

        if try!(lexer.skip_symbol(Symbol::Semicolon)) {
            return Ok(());
        }
        if !try!(lexer.skip_symbol(Symbol::Comma)) {
            let peek = try!(lexer.get());
            show_error_token(lexer, &peek, "expected ','");
            skip_until(lexer, Symbol::Semicolon);
            return Err(Error::Something);
        }
    }
}
fn read_opt_decl_or_stmt(lexer: &mut Lexer) -> ParseR<AST> {
    if try!(lexer.skip_symbol(Symbol::Semicolon)) {
        return Ok(AST::new(ASTKind::Compound(Vec::new()), Pos::new(0, 0)));
    }

    let peek_tok = try!(lexer.peek());
    if is_type(&peek_tok) {
        // variable declaration
        let mut stmts = Vec::new();
        let pos = lexer.get_cur_pos();
        try!(read_decl(lexer, &mut stmts));
        Ok(AST::new(ASTKind::Compound(stmts), pos))
    } else {
        read_stmt(lexer)
    }
}
// returns (declarator type, name, params{for function})
fn read_declarator(lexer: &mut Lexer, basety: Type) -> ParseR<(Type, String, Option<Vec<String>>)> {
    if try!(lexer.skip_symbol(Symbol::OpeningParen)) {
        let peek_tok = try!(lexer.peek());
        if is_type(&peek_tok) {
            let (ty, params) = try!(read_declarator_func(lexer, basety));
            return Ok((ty, "".to_string(), params));
        }

        // TODO: HUH? MAKES NO SENSE!!
        let mut buf: Vec<Token> = Vec::new();
        while !try!(lexer.skip_symbol(Symbol::ClosingParen)) {
            buf.push(try!(lexer.get()));
        }
        let t = try!(read_declarator_tail(lexer, basety));
        lexer.unget_all(&buf);
        return read_declarator(lexer, t.0);
    }

    if try!(lexer.skip_symbol(Symbol::Asterisk)) {
        try!(skip_type_qualifiers(lexer));
        return read_declarator(lexer, Type::Ptr(Rc::new(basety.clone())));
    }

    let tok = try!(lexer.get());

    if let &TokenKind::Identifier(ref name) = &tok.kind {
        let (ty, params) = try!(read_declarator_tail(lexer, basety));
        return Ok((ty, name.to_string(), params));
    }

    lexer.unget(tok);
    let (ty, params) = try!(read_declarator_tail(lexer, basety));
    Ok((ty, "".to_string(), params))
}
fn read_declarator_tail(lexer: &mut Lexer, basety: Type) -> ParseR<(Type, Option<Vec<String>>)> {
    if try!(lexer.skip_symbol(Symbol::OpeningBoxBracket)) {
        return Ok((try!(read_declarator_array(lexer, basety)), None));
    }
    if try!(lexer.skip_symbol(Symbol::OpeningParen)) {
        return read_declarator_func(lexer, basety);
    }
    Ok((basety, None))
}

fn read_declarator_array(lexer: &mut Lexer, basety: Type) -> ParseR<Type> {
    let len: i32;
    if try!(lexer.skip_symbol(Symbol::ClosingBoxBracket)) {
        len = -1;
    } else {
        len = match try!(read_expr(lexer)).eval_constexpr() {
            Ok(len) => len as i32,
            Err(Error::Something) => {
                let peek = try!(lexer.peek());
                show_error_token(lexer, &peek, "array size must be constant");
                0
            }
            Err(e) => return Err(e),
        };
        expect_symbol_error!(lexer, Symbol::ClosingBoxBracket, "expected ']'");
    }
    let ty = try!(read_declarator_tail(lexer, basety)).0;
    Ok(Type::Array(Rc::new(ty), len))
}
fn read_declarator_func(lexer: &mut Lexer, retty: Type) -> ParseR<(Type, Option<Vec<String>>)> {
    if try!(lexer.peek_keyword_token_is(Keyword::Void))
        && try!(lexer.next_symbol_token_is(Symbol::ClosingParen))
    {
        try!(lexer.expect_skip_keyword(Keyword::Void));
        try!(lexer.expect_skip_symbol(Symbol::ClosingParen));
        return Ok((Type::Func(Rc::new(retty), Vec::new(), false), None));
    }
    if try!(lexer.skip_symbol(Symbol::ClosingParen)) {
        return Ok((Type::Func(Rc::new(retty), Vec::new(), false), None));
    }

    let (paramtypes, paramnames, vararg) = try!(read_declarator_params(lexer));
    Ok((
        Type::Func(Rc::new(retty), paramtypes, vararg),
        Some(paramnames),
    ))
}
// returns (param types, param names, vararg?)
fn read_declarator_params(lexer: &mut Lexer) -> ParseR<(Vec<Type>, Vec<String>, bool)> {
    let mut paramtypes: Vec<Type> = Vec::new();
    let mut paramnames: Vec<String> = Vec::new();
    loop {
        if try!(lexer.skip_symbol(Symbol::Vararg)) {
            if paramtypes.len() == 0 {
                let peek = lexer.peek();
                show_error_token(
                    lexer,
                    &try!(peek),
                    "at least one param is required before '...'",
                );
                return Err(Error::Something);
            }
            expect_symbol_error!(lexer, Symbol::ClosingParen, "expected ')'");
            return Ok((paramtypes, paramnames, true));
        }

        let (ty, name) = try!(read_func_param(lexer));

        // meaning that reading parameter of defining function
        let mut env = ENV.lock().unwrap();
        if env.len() > 1 {
            env.back_mut().unwrap().insert(
                name.clone(),
                AST::new(ASTKind::Variable(ty.clone(), name.clone()), Pos::new(0, 0)),
            );
        }
        paramtypes.push(ty);
        paramnames.push(name);
        if try!(lexer.skip_symbol(Symbol::ClosingParen)) {
            return Ok((paramtypes, paramnames, false));
        }
        if !try!(lexer.skip_symbol(Symbol::Comma)) {
            let peek = lexer.peek();
            show_error_token(lexer, &try!(peek), "expected ','");
            skip_until(lexer, Symbol::ClosingParen);
            return Err(Error::Something);
        }
    }
}
fn read_func_param(lexer: &mut Lexer) -> ParseR<(Type, String)> {
    let basety = try!(read_type_spec(lexer)).0;
    let (ty, name, _) = try!(read_declarator(lexer, basety));
    match ty {
        Type::Array(subst, _) => Ok((Type::Ptr(subst), name)),
        Type::Func(_, _, _) => Ok((Type::Ptr(Rc::new(ty)), name)),
        _ => Ok((ty, name)),
    }
}
fn read_type_spec(lexer: &mut Lexer) -> ParseR<(Type, StorageClass, Qualifiers)> {
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
        let tok = try!(lexer.get());

        if kind.is_none() {
            if let &TokenKind::Identifier(ref maybe_userty_name) = &tok.kind {
                let maybe_userty = try!(get_typedef(lexer, maybe_userty_name));
                if maybe_userty.is_some() {
                    return Ok((maybe_userty.unwrap(), sclass, qualifiers));
                }
            }
        }
        if !matches!(tok.kind, TokenKind::Keyword(_)) {
            lexer.unget(tok);
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
                        let peek = lexer.peek();
                        show_error_token(lexer, &try!(peek), "type mismatch");
                    }
                    kind = Some(PrimitiveType::Void);
                }
                &Keyword::Char => {
                    if kind.is_some() {
                        let peek = lexer.peek();
                        show_error_token(lexer, &try!(peek), "type mismatch");
                    }
                    kind = Some(PrimitiveType::Char);
                }
                &Keyword::Int => {
                    if kind.is_some() {
                        let peek = lexer.peek();
                        show_error_token(lexer, &try!(peek), "type mismatch");
                    }
                    kind = Some(PrimitiveType::Int);
                }
                &Keyword::Float => {
                    if kind.is_some() {
                        let peek = lexer.peek();
                        show_error_token(lexer, &try!(peek), "type mismatch");
                    }
                    kind = Some(PrimitiveType::Float);
                }
                &Keyword::Double => {
                    if kind.is_some() {
                        let peek = lexer.peek();
                        show_error_token(lexer, &try!(peek), "type mismatch");
                    }
                    kind = Some(PrimitiveType::Double);
                }
                &Keyword::Signed => {
                    if sign.is_some() {
                        let peek = lexer.peek();
                        show_error_token(lexer, &try!(peek), "type mismatch");
                    };

                    sign = Some(Sign::Signed);
                }
                &Keyword::Unsigned => {
                    if sign.is_some() {
                        let peek = lexer.peek();
                        show_error_token(lexer, &try!(peek), "type mismatch");
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
                &Keyword::Struct => userty = Some(try!(read_struct_def(lexer))),
                &Keyword::Union => userty = Some(try!(read_union_def(lexer))),
                &Keyword::Enum => userty = Some(try!(read_enum_def(lexer))),
                _ => {}
            }
        } else {
            lexer.unget(tok);
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

fn read_struct_def(lexer: &mut Lexer) -> ParseR<Type> {
    read_rectype_def(lexer, true)
}
fn read_union_def(lexer: &mut Lexer) -> ParseR<Type> {
    read_rectype_def(lexer, false)
}
// rectype is abbreviation of 'record type'
fn read_rectype_tag(lexer: &mut Lexer) -> ParseR<Option<String>> {
    let maybe_tag = try!(lexer.get());
    if let TokenKind::Identifier(maybe_tag_name) = maybe_tag.kind {
        Ok(Some(maybe_tag_name))
    } else {
        lexer.unget(maybe_tag);
        Ok(None)
    }
}
fn read_rectype_def(lexer: &mut Lexer, is_struct: bool) -> ParseR<Type> {
    let tag = {
        let opt_tag = try!(read_rectype_tag(lexer));
        if opt_tag.is_some() {
            opt_tag.unwrap()
        } else {
            // if the rectype(struct|union) has no name(e.g. typedef struct { int a; } A),
            // generate a random name
            rand::thread_rng().gen_ascii_chars().take(8).collect()
        }
    };

    let fields = try!(read_rectype_fields(lexer));
    let mut tags = TAGS.lock().unwrap();
    let cur_tags = tags.back_mut().unwrap();

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
fn read_rectype_fields(lexer: &mut Lexer) -> ParseR<Vec<AST>> {
    if !try!(lexer.skip_symbol(Symbol::OpeningBrace)) {
        return Ok(Vec::new());
    }

    let mut decls: Vec<AST> = Vec::new();
    loop {
        let peek = try!(lexer.peek());
        if !is_type(&peek) {
            break;
        }
        let (basety, _, _) = try!(read_type_spec(lexer));
        loop {
            let (ty, name, _) = try!(read_declarator(lexer, basety.clone()));
            if try!(lexer.skip_symbol(Symbol::Colon)) {
                // TODO: for now, designated bitwidth ignore
                try!(read_expr(lexer));
            }
            decls.push(AST::new(
                ASTKind::VariableDecl(ty, name, StorageClass::Auto, None),
                lexer.get_cur_pos(),
            ));
            if try!(lexer.skip_symbol(Symbol::Comma)) {
                continue;
            } else {
                expect_symbol_error!(lexer, Symbol::Semicolon, "expected ';'");
            }
            break;
        }
    }
    expect_symbol_error!(lexer, Symbol::ClosingBrace, "expected '}'");
    Ok(decls)
}
fn read_enum_def(lexer: &mut Lexer) -> ParseR<Type> {
    let (tag, exist_tag) = {
        let opt_tag = try!(read_rectype_tag(lexer));
        if opt_tag.is_some() {
            (opt_tag.unwrap(), true)
        } else {
            ("".to_string(), false)
        }
    };
    let mut tags = TAGS.lock().unwrap();
    if exist_tag {
        match tags.back_mut().unwrap().get(tag.as_str()) {
            Some(&Type::Enum) => {}
            None => {}
            _ => {
                let peek = lexer.peek();
                show_error_token(lexer, &try!(peek), "undefined enum");
                return Err(Error::Something);
            }
        }
    }

    if !try!(lexer.skip_symbol(Symbol::OpeningBrace)) {
        if !exist_tag || !tags.back_mut().unwrap().contains_key(tag.as_str()) {
            let peek = lexer.peek();
            show_error_token(lexer, &try!(peek), "do not redefine enum");
            return Err(Error::Something);
        }
        return Ok(Type::Int(Sign::Signed));
    }

    if exist_tag {
        tags.back_mut().unwrap().insert(tag, Type::Enum);
    }

    let mut val = 0;
    loop {
        if try!(lexer.skip_symbol(Symbol::ClosingBrace)) {
            break;
        }
        let name = ident_val!(try!(lexer.get()));
        if try!(lexer.skip_symbol(Symbol::Assign)) {
            val = match try!(read_assign(lexer)).eval_constexpr() {
                Ok(val) => val,
                Err(Error::Something) => {
                    let peek = try!(lexer.peek());
                    show_error_token(lexer, &peek, "enum initialize value must be constant");
                    0
                }
                Err(e) => return Err(e),
            };
        }
        let constval = AST::new(ASTKind::Int(val, Bits::Bits32), lexer.get_cur_pos());
        val += 1;
        ENV.lock()
            .unwrap()
            .back_mut()
            .unwrap()
            .insert(name, constval);
        if try!(lexer.skip_symbol(Symbol::Comma)) {
            continue;
        }
        if try!(lexer.skip_symbol(Symbol::OpeningBrace)) {
            break;
        }
    }

    Ok(Type::Int(Sign::Signed))
}

pub fn read_expr(lexer: &mut Lexer) -> ParseR<AST> {
    read_comma(lexer)
}
pub fn read_opt_expr(lexer: &mut Lexer) -> ParseR<AST> {
    if try!(lexer.peek()).kind == TokenKind::Symbol(Symbol::Semicolon) {
        Ok(AST::new(ASTKind::Compound(Vec::new()), lexer.get_cur_pos()))
    } else {
        read_expr(lexer)
    }
}
fn read_comma(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_assign(lexer));
    while try!(lexer.skip_symbol(Symbol::Comma)) {
        let rhs = try!(read_assign(lexer));
        lhs = AST::new(
            ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Comma),
            lexer.get_cur_pos(),
        )
    }
    Ok(lhs)
}
fn read_assign(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_logor(lexer));
    if try!(lexer.skip_symbol(Symbol::Question)) {
        return read_ternary(lexer, lhs);
    }
    let assign = |lhs, rhs, pos| -> AST {
        AST::new(
            ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Assign),
            pos,
        )
    };
    loop {
        let tok = try!(lexer.get());
        match tok.kind {
            TokenKind::Symbol(Symbol::Assign) => {
                lhs = assign(lhs, try!(read_assign(lexer)), lexer.get_cur_pos());
            }
            TokenKind::Symbol(Symbol::AssignAdd) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Add,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignSub) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Sub,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignMul) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Mul,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignDiv) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Div,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignMod) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Rem,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignShl) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Shl,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignShr) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Shr,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignAnd) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::And,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            TokenKind::Symbol(Symbol::AssignOr) => {
                lhs = assign(
                    lhs.clone(),
                    AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(lhs),
                            Rc::new(try!(read_assign(lexer))),
                            node::CBinOps::Or,
                        ),
                        lexer.get_cur_pos(),
                    ),
                    lexer.get_cur_pos(),
                );
            }
            // TODO: implement more op
            _ => {
                lexer.unget(tok);
                break;
            }
        }
    }
    Ok(lhs)
}
fn read_ternary(lexer: &mut Lexer, cond: AST) -> ParseR<AST> {
    let mut then_expr = try!(read_expr(lexer));
    expect_symbol_error!(lexer, Symbol::Colon, "expected ':'");
    let mut else_expr = try!(read_assign(lexer));
    let then_ty = try!(get_expr_returning_ty(&then_expr));
    let else_ty = try!(get_expr_returning_ty(&else_expr));
    if then_ty.is_arith_ty() && else_ty.is_arith_ty() {
        let ty = usual_binary_ty_cov(then_ty, else_ty);
        then_expr = cast_ast(&then_expr, &ty);
        else_expr = cast_ast(&else_expr, &ty);
    }
    Ok(AST::new(
        ASTKind::TernaryOp(Rc::new(cond), Rc::new(then_expr), Rc::new(else_expr)),
        lexer.get_cur_pos(),
    ))
}
fn read_logor(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_logand(lexer));
    while try!(lexer.skip_symbol(Symbol::LOr)) {
        let rhs = try!(read_logand(lexer));
        lhs = AST::new(
            ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LOr),
            lexer.get_cur_pos(),
        );
    }
    Ok(lhs)
}
fn read_logand(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_or(lexer));
    while try!(lexer.skip_symbol(Symbol::LAnd)) {
        let rhs = try!(read_or(lexer));
        lhs = AST::new(
            ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LAnd),
            lexer.get_cur_pos(),
        );
    }
    Ok(lhs)
}
fn read_or(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_xor(lexer));
    while try!(lexer.skip_symbol(Symbol::Or)) {
        let rhs = try!(read_xor(lexer));
        lhs = AST::new(
            ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Or),
            lexer.get_cur_pos(),
        );
    }
    Ok(lhs)
}
fn read_xor(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_and(lexer));
    while try!(lexer.skip_symbol(Symbol::Xor)) {
        let rhs = try!(read_and(lexer));
        lhs = AST::new(
            ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Xor),
            lexer.get_cur_pos(),
        );
    }
    Ok(lhs)
}
fn read_and(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_eq_ne(lexer));
    while try!(lexer.skip_symbol(Symbol::Ampersand)) {
        let rhs = try!(read_eq_ne(lexer));
        lhs = AST::new(
            ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::And),
            lexer.get_cur_pos(),
        );
    }
    Ok(lhs)
}
fn read_eq_ne(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_relation(lexer));
    loop {
        if try!(lexer.skip_symbol(Symbol::Eq)) {
            let rhs = try!(read_relation(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Eq),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Ne)) {
            let rhs = try!(read_relation(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ne),
                lexer.get_cur_pos(),
            );
        } else {
            break;
        }
    }
    Ok(lhs)
}
fn read_relation(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_shl_shr(lexer));
    loop {
        if try!(lexer.skip_symbol(Symbol::Lt)) {
            let rhs = try!(read_shl_shr(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Lt),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Le)) {
            let rhs = try!(read_shl_shr(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Le),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Gt)) {
            let rhs = try!(read_shl_shr(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Gt),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Ge)) {
            let rhs = try!(read_shl_shr(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ge),
                lexer.get_cur_pos(),
            );
        } else {
            break;
        }
    }
    Ok(lhs)
}
fn read_shl_shr(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_add_sub(lexer));
    loop {
        if try!(lexer.skip_symbol(Symbol::Shl)) {
            let rhs = try!(read_add_sub(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shl),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Shr)) {
            let rhs = try!(read_add_sub(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shr),
                lexer.get_cur_pos(),
            );
        } else {
            break;
        }
    }
    Ok(lhs)
}
fn read_add_sub(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_mul_div_rem(lexer));
    loop {
        if try!(lexer.skip_symbol(Symbol::Add)) {
            let rhs = try!(read_mul_div_rem(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Add),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Sub)) {
            let rhs = try!(read_mul_div_rem(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Sub),
                lexer.get_cur_pos(),
            );
        } else {
            break;
        }
    }
    Ok(lhs)
}
fn read_mul_div_rem(lexer: &mut Lexer) -> ParseR<AST> {
    let mut lhs = try!(read_cast(lexer));
    loop {
        if try!(lexer.skip_symbol(Symbol::Asterisk)) {
            let rhs = try!(read_cast(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Mul),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Div)) {
            let rhs = try!(read_cast(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Div),
                lexer.get_cur_pos(),
            );
        } else if try!(lexer.skip_symbol(Symbol::Mod)) {
            let rhs = try!(read_cast(lexer));
            lhs = AST::new(
                ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Rem),
                lexer.get_cur_pos(),
            );
        } else {
            break;
        }
    }
    Ok(lhs)
}
fn read_cast(lexer: &mut Lexer) -> ParseR<AST> {
    let tok = try!(lexer.get());
    let peek = try!(lexer.peek());
    if tok.kind == TokenKind::Symbol(Symbol::OpeningParen) && is_type(&peek) {
        let basety = try!(read_type_spec(lexer)).0;
        let ty = try!(read_declarator(lexer, basety)).0;
        expect_symbol_error!(lexer, Symbol::ClosingParen, "expected ')'");
        return Ok(AST::new(
            ASTKind::TypeCast(Rc::new(try!(read_cast(lexer))), ty),
            lexer.get_cur_pos(),
        ));
    } else {
        lexer.unget(tok);
    }
    read_unary(lexer)
}
fn read_unary(lexer: &mut Lexer) -> ParseR<AST> {
    let tok = try!(lexer.get());
    match tok.kind {
        TokenKind::Symbol(Symbol::Not) => {
            return Ok(AST::new(
                ASTKind::UnaryOp(Rc::new(try!(read_cast(lexer))), node::CUnaryOps::LNot),
                lexer.get_cur_pos(),
            ))
        }
        TokenKind::Symbol(Symbol::BitwiseNot) => {
            return Ok(AST::new(
                ASTKind::UnaryOp(Rc::new(try!(read_cast(lexer))), node::CUnaryOps::BNot),
                lexer.get_cur_pos(),
            ))
        }
        TokenKind::Symbol(Symbol::Add) => {
            return Ok(AST::new(
                ASTKind::UnaryOp(Rc::new(try!(read_cast(lexer))), node::CUnaryOps::Plus),
                lexer.get_cur_pos(),
            ))
        }
        TokenKind::Symbol(Symbol::Sub) => {
            return Ok(AST::new(
                ASTKind::UnaryOp(Rc::new(try!(read_cast(lexer))), node::CUnaryOps::Minus),
                lexer.get_cur_pos(),
            ))
        }
        TokenKind::Symbol(Symbol::Inc) => {
            let pos = lexer.get_cur_pos();
            let var = try!(read_cast(lexer));
            return Ok(AST::new(
                ASTKind::BinaryOp(
                    Rc::new(var.clone()),
                    Rc::new(AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(var),
                            Rc::new(AST::new(ASTKind::Int(1, Bits::Bits32), pos.clone())),
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
            let pos = lexer.get_cur_pos();
            let var = try!(read_cast(lexer));
            return Ok(AST::new(
                ASTKind::BinaryOp(
                    Rc::new(var.clone()),
                    Rc::new(AST::new(
                        ASTKind::BinaryOp(
                            Rc::new(var),
                            Rc::new(AST::new(ASTKind::Int(1, Bits::Bits32), pos.clone())),
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
                ASTKind::UnaryOp(Rc::new(try!(read_cast(lexer))), node::CUnaryOps::Deref),
                lexer.get_cur_pos(),
            ))
        }
        TokenKind::Symbol(Symbol::Ampersand) => {
            return Ok(AST::new(
                ASTKind::UnaryOp(Rc::new(try!(read_cast(lexer))), node::CUnaryOps::Addr),
                lexer.get_cur_pos(),
            ))
        }
        TokenKind::Symbol(Symbol::Sizeof) => {
            // TODO: must fix this sloppy implementation
            return read_sizeof(lexer);
        }
        _ => {}
    }
    lexer.unget(tok);
    read_postfix(lexer)
}
fn read_sizeof(lexer: &mut Lexer) -> ParseR<AST> {
    let tok = try!(lexer.get());
    let peek = try!(lexer.peek());
    if matches!(tok.kind, TokenKind::Symbol(Symbol::OpeningParen)) && is_type(&peek) {
        let (basety, _, _) = try!(read_type_spec(lexer));
        let (ty, _, _) = try!(read_declarator(lexer, basety));
        try!(lexer.skip_symbol(Symbol::ClosingParen));
        return Ok(AST::new(
            ASTKind::Int(ty.calc_size() as i64, Bits::Bits32),
            lexer.get_cur_pos(),
        ));
    }
    lexer.unget(tok);
    let expr = try!(read_unary(lexer));
    Ok(AST::new(
        ASTKind::Int(try!(calc_sizeof(&expr)) as i64, Bits::Bits32),
        lexer.get_cur_pos(),
    ))
}
fn read_postfix(lexer: &mut Lexer) -> ParseR<AST> {
    let mut ast = try!(read_primary(lexer));
    loop {
        if try!(lexer.skip_symbol(Symbol::OpeningParen)) {
            ast = try!(read_func_call(lexer, ast));
            continue;
        }
        if try!(lexer.skip_symbol(Symbol::OpeningBoxBracket)) {
            ast = AST::new(
                ASTKind::Load(Rc::new(try!(read_index(lexer, ast)))),
                lexer.get_cur_pos(),
            );
            continue;
        }
        if try!(lexer.skip_symbol(Symbol::Point)) {
            ast = AST::new(
                ASTKind::Load(Rc::new(try!(read_field(lexer, ast)))),
                lexer.get_cur_pos(),
            );
            continue;
        }
        if try!(lexer.skip_symbol(Symbol::Arrow)) {
            let pos = lexer.get_cur_pos();
            let field = try!(read_field(
                lexer,
                AST::new(
                    ASTKind::UnaryOp(Rc::new(ast), node::CUnaryOps::Deref),
                    pos.clone()
                )
            ));
            ast = AST::new(ASTKind::Load(Rc::new(field)), pos);
            continue;
        }
        if try!(lexer.skip_symbol(Symbol::Inc)) {
            return Ok(AST::new(
                ASTKind::UnaryOp(Rc::new(ast), node::CUnaryOps::Inc),
                lexer.get_cur_pos(),
            ));
        }
        if try!(lexer.skip_symbol(Symbol::Dec)) {
            return Ok(AST::new(
                ASTKind::UnaryOp(Rc::new(ast), node::CUnaryOps::Dec),
                lexer.get_cur_pos(),
            ));
        }
        break;
    }
    Ok(ast)
}
fn read_func_call(lexer: &mut Lexer, f: AST) -> ParseR<AST> {
    let pos = lexer.get_cur_pos();
    let mut args = Vec::new();
    if !try!(lexer.skip_symbol(Symbol::ClosingParen)) {
        loop {
            match read_assign(lexer) {
                Ok(arg) => args.push(arg),
                Err(_) => {}
            }

            if try!(lexer.skip_symbol(Symbol::ClosingParen)) {
                break;
            }
            if !try!(lexer.skip_symbol(Symbol::Comma)) {
                let peek = lexer.peek();
                show_error_token(lexer, &try!(peek), "expected ','");
                skip_until(lexer, Symbol::ClosingParen);
                return Err(Error::Something);
            }
        }
    }

    // TODO: do really have to support constexpr?
    // if let Some(name) = codegen::retrieve_from_load(&f).get_variable_name() {
    //     if CONSTEXPR_FUNC_MAP.lock().unwrap().contains(name) {
    //         let mut are_args_const = true;
    //         for arg in &args {
    //             if are_args_const && !arg.is_const() {
    //                 are_args_const = false;
    //                 break;
    //             }
    //         }
    //         if are_args_const {
    //             unsafe {
    //                 if let Ok(ret) = CODEGEN.lock().unwrap().call_constexpr_func(name, &args) {
    //                     return Ok(ret);
    //                 }
    //             }
    //         }
    //     }
    // }
    //
    Ok(AST::new(ASTKind::FuncCall(Rc::new(f), args), pos))
}
fn read_index(lexer: &mut Lexer, ast: AST) -> ParseR<AST> {
    let idx = try!(read_expr(lexer));
    expect_symbol_error!(lexer, Symbol::ClosingBoxBracket, "expected ']'");
    Ok(AST::new(
        ASTKind::BinaryOp(Rc::new(ast), Rc::new(idx), node::CBinOps::Add),
        lexer.get_cur_pos(),
    ))
}

fn read_field(lexer: &mut Lexer, ast: AST) -> ParseR<AST> {
    let field = try!(lexer.get());
    if !matches!(field.kind, TokenKind::Identifier(_)) {
        let peek = lexer.peek();
        show_error_token(lexer, &try!(peek), "expected field name");
        return Err(Error::Something);
    }

    let field_name = ident_val!(field);
    Ok(AST::new(
        ASTKind::StructRef(Rc::new(ast), field_name),
        lexer.get_cur_pos(),
    ))
}

fn read_const_array(lexer: &mut Lexer) -> ParseR<AST> {
    let mut elems = Vec::new();
    loop {
        elems.push(try!(read_assign(lexer)));
        if try!(lexer.skip_symbol(Symbol::ClosingBrace)) {
            break;
        }
        if !try!(lexer.skip_symbol(Symbol::Comma)) {
            let peek = lexer.peek();
            show_error_token(lexer, &try!(peek), "expected ','");
            skip_until(lexer, Symbol::ClosingBrace);
            return Err(Error::Something);
        }
    }
    Ok(AST::new(ASTKind::ConstArray(elems), lexer.get_cur_pos()))
}
fn read_primary(lexer: &mut Lexer) -> ParseR<AST> {
    let tok = match lexer.get() {
        Ok(tok) => tok,
        Err(_) => {
            let peek = lexer.peek();
            show_error_token(
                lexer,
                &try!(peek),
                "expected primary(number, string...), but reach EOF",
            );
            return Err(Error::Something);
        }
    };

    match tok.kind.clone() {
        TokenKind::IntNumber(n, bits) => Ok(AST::new(ASTKind::Int(n, bits), lexer.get_cur_pos())),
        TokenKind::FloatNumber(f) => Ok(AST::new(ASTKind::Float(f), lexer.get_cur_pos())),
        TokenKind::Identifier(ident) => {
            if let Some(ast) = ENV.lock().unwrap().back().unwrap().get(ident.as_str()) {
                return match ast.kind {
                    ASTKind::Variable(_, _) => Ok(AST::new(
                        ASTKind::Load(Rc::new((*ast).clone())),
                        lexer.get_cur_pos(),
                    )),
                    _ => Ok((*ast).clone()),
                };
            }
            show_error_token(
                lexer,
                &tok,
                format!("not found the variable or function '{}'", ident).as_str(),
            );
            Err(Error::Something)
        }
        TokenKind::String(s) => Ok(AST::new(ASTKind::String(s), lexer.get_cur_pos())),
        TokenKind::Char(ch) => Ok(AST::new(ASTKind::Char(ch as i32), lexer.get_cur_pos())),
        TokenKind::Symbol(sym) => match sym {
            Symbol::OpeningParen => {
                let expr = read_expr(lexer);
                if !try!(lexer.skip_symbol(Symbol::ClosingParen)) {
                    show_error_token(lexer, &tok, "expected ')'");
                }
                expr
            }
            Symbol::OpeningBrace => read_const_array(lexer),
            _ => {
                show_error_token(
                    lexer,
                    &tok,
                    format!("expected primary section, but got {:?}", tok.kind).as_str(),
                );
                Err(Error::Something)
            }
        },
        _ => {
            show_error_token(
                lexer,
                &tok,
                format!("read_primary unknown token {:?}", tok.kind).as_str(),
            );
            Err(Error::Something)
        }
    }
}

fn usual_binary_ty_cov(lhs: Type, rhs: Type) -> Type {
    if lhs.priority() < rhs.priority() {
        rhs
    } else {
        lhs
    }
}
fn get_binary_expr_ty(lhs: &AST, rhs: &AST, op: &node::CBinOps) -> ParseR<Type> {
    fn cast(ty: Type) -> Type {
        match ty {
            Type::Array(elem_ty, _) => Type::Ptr(elem_ty),
            Type::Func(_, _, _) => Type::Ptr(Rc::new(ty)),
            _ => ty,
        }
    }
    let lhs_ty = cast(try!(get_expr_returning_ty(lhs)));
    let rhs_ty = cast(try!(get_expr_returning_ty(rhs)));
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
    return Ok(usual_binary_ty_cov(lhs_ty, rhs_ty));
}
fn get_expr_returning_ty(ast: &AST) -> ParseR<Type> {
    let size = match ast.kind {
        ASTKind::Int(_, Bits::Bits32) => Type::Int(Sign::Signed),
        ASTKind::Int(_, Bits::Bits64) => Type::Long(Sign::Signed),
        ASTKind::Float(_) => Type::Double,
        ASTKind::Char(_) => Type::Char(Sign::Signed),
        ASTKind::String(ref s) => {
            Type::Array(Rc::new(Type::Char(Sign::Signed)), s.len() as i32 + 1)
        }
        ASTKind::Load(ref v) => (*try!(get_expr_returning_ty(&*v)).get_elem_ty().unwrap()).clone(),
        ASTKind::Variable(ref ty, _) => Type::Ptr(Rc::new((*ty).clone())),
        ASTKind::UnaryOp(_, node::CUnaryOps::LNot) => Type::Int(Sign::Signed),
        ASTKind::UnaryOp(ref expr, node::CUnaryOps::Minus)
        | ASTKind::UnaryOp(ref expr, node::CUnaryOps::Inc)
        | ASTKind::UnaryOp(ref expr, node::CUnaryOps::Dec)
        | ASTKind::UnaryOp(ref expr, node::CUnaryOps::BNot) => try!(get_expr_returning_ty(&*expr)),
        ASTKind::UnaryOp(ref expr, node::CUnaryOps::Deref) => {
            (*try!(get_expr_returning_ty(&*expr)).get_elem_ty().unwrap()).clone()
        }
        ASTKind::UnaryOp(ref expr, node::CUnaryOps::Addr) => {
            Type::Ptr(Rc::new(try!(get_expr_returning_ty(&*expr))))
        }
        ASTKind::StructRef(ref expr, ref name) => {
            let ty = try!(get_expr_returning_ty(expr));
            Type::Ptr(Rc::new((*ty.get_field_ty(name.as_str()).unwrap()).clone()))
        }
        ASTKind::TypeCast(_, ref ty) => ty.clone(),
        ASTKind::BinaryOp(ref lhs, ref rhs, ref op) => try!(get_binary_expr_ty(&*lhs, &*rhs, &*op)),
        ASTKind::TernaryOp(_, ref then, _) => try!(get_expr_returning_ty(&*then)),
        ASTKind::FuncCall(ref func, _) => {
            let func_ty = try!(get_expr_returning_ty(func));
            (*func_ty.get_return_ty().unwrap()).clone()
        }
        _ => panic!(format!("unsupported: {:?}", ast.kind)),
    };
    Ok(size)
}
fn calc_sizeof(ast: &AST) -> ParseR<usize> {
    let ty = try!(get_expr_returning_ty(ast));
    Ok(ty.calc_size())
}

fn cast_ast(expr: &AST, ty: &Type) -> AST {
    AST::new(
        ASTKind::TypeCast(Rc::new(expr.clone()), ty.clone()),
        expr.pos.clone(),
    )
}
