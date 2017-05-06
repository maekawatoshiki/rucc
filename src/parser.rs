use lexer::{Lexer, Token, TokenKind};
use node::AST;
use node;
use error;
use types::{Type, Sign};

use std::fs::OpenOptions;
use std::io::prelude::*;
use std::{str, u32};
use std::rc::Rc;

pub fn run_file(filename: String) -> Vec<AST> {
    let mut nodes: Vec<AST> = Vec::new();
    let mut file = OpenOptions::new()
        .read(true)
        .open(filename.to_string())
        .unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s);
    let mut lexer = Lexer::new(filename.to_string(), s.as_str());
    loop {
        let tok = lexer.get();
        match tok {
            Some(t) => {
                println!("t:{}{}", if t.space { " " } else { "" }, t.val);
            }
            None => break,
        }
    }

    // Debug: (parsing again is big cost?)
    lexer = Lexer::new(filename.to_string(), s.as_str());
    read_toplevel(&mut lexer, &mut nodes);
    nodes
}

pub fn run(input: String) -> Vec<AST> {
    let mut lexer = Lexer::new("__input__".to_string(), input.as_str());
    let mut nodes: Vec<AST> = Vec::new();
    loop {
        if lexer.peek().is_none() {
            break;
        }
        read_toplevel(&mut lexer, &mut nodes);
    }
    nodes
}

fn read_toplevel(lexer: &mut Lexer, ast: &mut Vec<AST>) {
    if is_function_def(lexer) {
        ast.push(read_func_def(lexer));
    } else {
        read_decl(lexer, ast);
    }
}

fn read_func_def(lexer: &mut Lexer) -> AST {
    // TODO: IMPLEMENT
    let retty = read_type_spec(lexer);
    let (functy, name, param_names) = read_declarator(lexer, retty);
    println!("functy: {:?}", functy);

    lexer.expect_skip("{");
    let body = read_func_body(lexer, &functy);
    AST::FuncDef(functy,
                 if param_names.is_none() {
                     Vec::new()
                 } else {
                     param_names.unwrap()
                 },
                 name,
                 Rc::new(body))
}

fn read_func_body(lexer: &mut Lexer, _functy: &Type) -> AST {
    read_compound_stmt(lexer)
}

fn read_compound_stmt(lexer: &mut Lexer) -> AST {
    let mut stmts: Vec<AST> = Vec::new();
    loop {
        if lexer.skip("}") {
            break;
        }

        if is_type(&lexer.peek_e()) {
            // variable declaration
            read_decl(lexer, &mut stmts);
        } else {
            stmts.push(read_stmt(lexer));
        }
    }
    AST::Block(stmts)
}

fn read_stmt(lexer: &mut Lexer) -> AST {
    let tok = lexer.get_e();
    match tok.val.as_str() {
        "{" => return read_compound_stmt(lexer),
        "return" => return read_return_stmt(lexer),
        _ => {}
    }
    lexer.unget(tok);
    let expr = read_expr(lexer);
    lexer.expect_skip(";");
    expr
}

fn read_return_stmt(lexer: &mut Lexer) -> AST {
    let retval = read_expr(lexer);
    let retast = AST::Return(Rc::new(retval));
    lexer.expect_skip(";");
    retast
}

fn is_function_def(lexer: &mut Lexer) -> bool {
    let mut buf: Vec<Token> = Vec::new();
    let mut is_funcdef = false;

    loop {
        let mut tok = lexer.get().unwrap();
        buf.push(tok.clone());

        if tok.val == ";" {
            break;
        }

        if is_type(&tok) {
            continue;
        }

        if tok.val == "(" {
            skip_brackets(lexer, &mut buf);
            continue;
        }

        if tok.kind != TokenKind::Identifier {
            continue;
        }

        if lexer.peek().unwrap().val != "(" {
            continue;
        }

        buf.push(lexer.get().unwrap());
        skip_brackets(lexer, &mut buf);

        tok = lexer.peek().unwrap();
        is_funcdef = tok.val.as_str() == "{";
        break;
    }

    lexer.unget_all(buf);
    is_funcdef
}

fn skip_brackets(lexer: &mut Lexer, buf: &mut Vec<Token>) {
    loop {
        let tok = lexer.get().unwrap();
        buf.push(tok.clone());

        match tok.val.as_str() {
            "(" => skip_brackets(lexer, buf),
            ")" => break,
            _ => {}
        }
    }
}

fn is_type(token: &Token) -> bool {
    if token.kind != TokenKind::Identifier {
        return false;
    }
    match token.val.as_str() {
        "void" | "signed" | "unsigned" | "char" | "int" | "short" | "long" | "float" |
        "double" | "struct" | "union" | "extern" | "const" | "volatile" => true,
        _ => false,
    }
}

fn read_decl_init(lexer: &mut Lexer) -> AST {
    // TODO: implement for like 'int a[] = {...}, char *s="str";'
    read_assign(lexer)
}

fn skip_type_qualifiers(lexer: &mut Lexer) {
    while lexer.skip("const") || lexer.skip("volatile") || lexer.skip("restrict") {}
}
fn read_decl(lexer: &mut Lexer, ast: &mut Vec<AST>) {
    let basety = read_type_spec(lexer);
    if lexer.skip(";") {
        return;
    }

    loop {
        let (ty, name, _) = read_declarator(lexer, basety.clone());

        let init = if lexer.skip("=") {
            Some(Rc::new(read_decl_init(lexer)))
        } else {
            None
        };
        ast.push(AST::VariableDecl(ty, name, init));

        if lexer.skip(";") {
            return;
        }
        lexer.expect_skip(",");
    }
}

// returns (declarator type, name, params{for function})
fn read_declarator(lexer: &mut Lexer, basety: Type) -> (Type, String, Option<Vec<String>>) {
    if lexer.skip("(") {
        if is_type(&lexer.peek().unwrap()) {
            let (ty, params) = read_declarator_func(lexer, basety);
            return (ty, "".to_string(), params);
        }

        // TODO: HUH? MAKES NO SENSE!!
        let mut buf: Vec<Token> = Vec::new();
        while !lexer.skip(")") {
            buf.push(lexer.get().unwrap());
        }
        let t = read_declarator_tail(lexer, basety);
        lexer.unget_all(buf);
        return read_declarator(lexer, t.0);
    }

    if lexer.skip("*") {
        skip_type_qualifiers(lexer);
        return read_declarator(lexer, Type::Ptr(Rc::new(basety)));
    }

    let tok = lexer.get().unwrap();

    if tok.kind == TokenKind::Identifier {
        let name = tok.val;
        let (ty, params) = read_declarator_tail(lexer, basety);
        return (ty, name, params);
    }

    lexer.unget(tok);
    let (ty, params) = read_declarator_tail(lexer, basety);
    (ty, "".to_string(), params)
}

fn read_declarator_tail(lexer: &mut Lexer, basety: Type) -> (Type, Option<Vec<String>>) {
    if lexer.skip("[") {
        return (read_declarator_array(lexer, basety), None);
    }
    if lexer.skip("(") {
        return read_declarator_func(lexer, basety);
    }
    (basety, None)
}

fn read_declarator_array(lexer: &mut Lexer, basety: Type) -> Type {
    let len: i32;
    if lexer.skip("]") {
        len = -1;
    } else {
        len = read_expr(lexer).eval_constexpr();
        lexer.expect_skip("]");
    }
    let ty = read_declarator_tail(lexer, basety).0;
    Type::Array(Rc::new(ty), len)
}

fn read_declarator_func(lexer: &mut Lexer, retty: Type) -> (Type, Option<Vec<String>>) {
    if lexer.skip("void") {
        lexer.expect_skip(")");
        return (Type::Func(Rc::new(retty), Vec::new(), false), None);
    }
    if lexer.skip(")") {
        return (Type::Func(Rc::new(retty), Vec::new(), false), None);
    }

    let (paramtypes, paramnames, vararg) = read_declarator_params(lexer);
    (Type::Func(Rc::new(retty), paramtypes.clone(), vararg), Some(paramnames))
}

// returns (param types, param names, vararg?)
fn read_declarator_params(lexer: &mut Lexer) -> (Vec<Type>, Vec<String>, bool) {
    let mut paramtypes: Vec<Type> = Vec::new();
    let mut paramnames: Vec<String> = Vec::new();
    loop {
        if lexer.skip("...") {
            if paramtypes.len() == 0 {
                error::error_exit(lexer.cur_line,
                                  "at least one param is required before '...'");
            }
            lexer.expect_skip(")");
            return (paramtypes, paramnames, true);
        }

        let (ty, name) = read_func_param(lexer);
        paramtypes.push(ty);
        paramnames.push(name);
        if lexer.skip(")") {
            return (paramtypes, paramnames, false);
        }
        lexer.expect_skip(",");
    }
}

fn read_func_param(lexer: &mut Lexer) -> (Type, String) {
    let basety = read_type_spec(lexer);
    let (ty, name, _) = read_declarator(lexer, basety);
    match ty {
        Type::Array(subst, _) => return (Type::Ptr(subst), name),
        Type::Func(_, _, _) => return (Type::Ptr(Rc::new(ty)), name),
        _ => {}
    }
    (ty, name)
}

fn read_type_spec(lexer: &mut Lexer) -> Type {
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

    let err_kind = |lexer: &Lexer, kind: Option<PrimitiveType>| if kind.is_some() {
        error::error_exit(lexer.cur_line, "type mismatch");
    };
    let err_sign = |lexer: &Lexer, sign: Option<Sign>| if sign.is_some() {
        error::error_exit(lexer.cur_line, "type mismatch");
    };

    loop {
        let tok = lexer
            .get()
            .or_else(|| error::error_exit(lexer.cur_line, "expect types but reach EOF"))
            .unwrap();

        // TODO: check whether typedef

        match tok.val.as_str() {
            "const" | "volatile" | "inline" | "noreturn" => {}
            "void" => {
                err_kind(&lexer, kind);
                kind = Some(PrimitiveType::Void);
            }
            "char" => {
                err_kind(&lexer, kind);
                kind = Some(PrimitiveType::Char);
            }
            "int" => {
                err_kind(&lexer, kind);
                kind = Some(PrimitiveType::Int);
            }
            "float" => {
                err_kind(&lexer, kind);
                kind = Some(PrimitiveType::Float);
            }
            "double" => {
                err_kind(&lexer, kind);
                kind = Some(PrimitiveType::Double);
            }
            "signed" => {
                err_sign(&lexer, sign);
                sign = Some(Sign::Signed);
            }
            "unsigned" => {
                err_sign(&lexer, sign);
                sign = Some(Sign::Unsigned);
            }
            "short" => size = Size::Short,
            "long" => {
                if size == Size::Normal {
                    size = Size::Long;
                } else if size == Size::Long {
                    size = Size::LLong;
                }
            }
            _ => {
                lexer.unget(tok);
                break;
            }
        }
    }

    // if sign is not expected,
    //  default is Signed
    if sign.is_none() {
        sign = Some(Sign::Signed);
    }

    let mut ty: Option<Type> = None;
    // e.g. kind is None => 'signed var;' or 'unsigned var;'
    if kind.is_some() {
        match kind.unwrap() {
            PrimitiveType::Void => ty = Some(Type::Void),
            PrimitiveType::Char => ty = Some(Type::Char(sign.clone().unwrap())),
            PrimitiveType::Float => ty = Some(Type::Float),
            PrimitiveType::Double => ty = Some(Type::Double),
            _ => {}
        }
        if ty.is_some() {
            return ty.unwrap();
        }
    }

    match size {
        Size::Short => ty = Some(Type::Short(sign.clone().unwrap())),
        Size::Normal => ty = Some(Type::Int(sign.clone().unwrap())),
        Size::Long => ty = Some(Type::Long(sign.clone().unwrap())),
        Size::LLong => ty = Some(Type::LLong(sign.clone().unwrap())),
    }

    assert!(ty.is_some(), "ty is None!");
    ty.unwrap()
}

pub fn read_expr(lexer: &mut Lexer) -> AST {
    let lhs = read_comma(lexer);
    lhs
}
////////// operators start from here
fn read_comma(lexer: &mut Lexer) -> AST {
    let mut lhs = read_assign(lexer);
    while lexer.skip(",") {
        let rhs = read_assign(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Comma);
    }
    lhs
}
fn read_assign(lexer: &mut Lexer) -> AST {
    let mut lhs = read_logor(lexer);
    while lexer.skip("=") {
        let rhs = read_logor(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Assign);
    }
    lhs
}
fn read_logor(lexer: &mut Lexer) -> AST {
    let mut lhs = read_logand(lexer);
    while lexer.skip("||") {
        let rhs = read_logand(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LOr);
    }
    lhs
}
fn read_logand(lexer: &mut Lexer) -> AST {
    let mut lhs = read_or(lexer);
    while lexer.skip("&&") {
        let rhs = read_or(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LAnd);
    }
    lhs
}
fn read_or(lexer: &mut Lexer) -> AST {
    let mut lhs = read_xor(lexer);
    while lexer.skip("|") {
        let rhs = read_xor(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Or);
    }
    lhs
}
fn read_xor(lexer: &mut Lexer) -> AST {
    let mut lhs = read_and(lexer);
    while lexer.skip("^") {
        let rhs = read_and(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Xor);
    }
    lhs
}
fn read_and(lexer: &mut Lexer) -> AST {
    let mut lhs = read_eq_ne(lexer);
    while lexer.skip("&") {
        let rhs = read_eq_ne(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::And);
    }
    lhs
}
fn read_eq_ne(lexer: &mut Lexer) -> AST {
    let mut lhs = read_relation(lexer);
    loop {
        if lexer.skip("==") {
            let rhs = read_relation(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Eq);
        } else if lexer.skip("!=") {
            let rhs = read_relation(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ne);
        } else {
            break;
        }
    }
    lhs
}
fn read_relation(lexer: &mut Lexer) -> AST {
    let mut lhs = read_shl_shr(lexer);
    loop {
        if lexer.skip("<") {
            let rhs = read_shl_shr(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Lt);
        } else if lexer.skip("<=") {
            let rhs = read_shl_shr(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Le);
        } else if lexer.skip(">") {
            let rhs = read_shl_shr(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Gt);
        } else if lexer.skip(">=") {
            let rhs = read_shl_shr(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ge);
        } else {
            break;
        }
    }
    lhs
}
fn read_shl_shr(lexer: &mut Lexer) -> AST {
    let mut lhs = read_add_sub(lexer);
    loop {
        if lexer.skip("<<") {
            let rhs = read_add_sub(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shl);
        } else if lexer.skip(">>") {
            let rhs = read_add_sub(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shr);
        } else {
            break;
        }
    }
    lhs
}
fn read_add_sub(lexer: &mut Lexer) -> AST {
    let mut lhs = read_mul_div_rem(lexer);
    loop {
        if lexer.skip("+") {
            let rhs = read_mul_div_rem(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Add);
        } else if lexer.skip("-") {
            let rhs = read_mul_div_rem(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Sub);
        } else {
            break;
        }
    }
    lhs
}
fn read_mul_div_rem(lexer: &mut Lexer) -> AST {
    let mut lhs = read_cast(lexer);
    loop {
        if lexer.skip("*") {
            let rhs = read_cast(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Mul);
        } else if lexer.skip("/") {
            let rhs = read_cast(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Div);
        } else if lexer.skip("%") {
            let rhs = read_cast(lexer);
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Rem);
        } else {
            break;
        }
    }
    lhs
}
fn read_cast(lexer: &mut Lexer) -> AST {
    read_unary(lexer)
}
fn read_unary(lexer: &mut Lexer) -> AST {
    let tok = lexer
        .get()
        .or_else(|| error::error_exit(lexer.cur_line, "expected unary op"))
        .unwrap();
    if tok.kind == TokenKind::Symbol {
        match tok.val.as_str() { 
            "!" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::LNot),
            "~" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::BNot),
            "+" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Plus),
            "-" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Minus),
            "++" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Inc),
            "--" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Dec),
            "*" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Deref),
            "&" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Addr),
            _ => {}
        }
    }
    lexer.unget(tok);
    read_postfix(lexer)
}

fn read_postfix(lexer: &mut Lexer) -> AST {
    let mut ast = read_primary(lexer);
    loop {
        if lexer.skip("(") {
            ast = read_func_call(lexer, ast);
            continue;
        }
        if lexer.skip("[") {
            ast = read_index(lexer, ast);
            continue;
        }
        if lexer.skip(".") {
            ast = read_field(lexer, ast);
        }
        if lexer.skip("->") {
            ast = read_field(lexer, AST::UnaryOp(Rc::new(ast), node::CUnaryOps::Deref));
        }
        // TODO: impelment inc and dec
        break;
    }
    ast
}

fn read_func_call(lexer: &mut Lexer, f: AST) -> AST {
    let mut args: Vec<AST> = Vec::new();
    loop {
        let arg = read_assign(lexer);
        args.push(arg);

        if lexer.skip(")") {
            break;
        }
        lexer.expect_skip(",");
    }
    AST::FuncCall(Rc::new(f), args)
}

fn read_index(lexer: &mut Lexer, ast: AST) -> AST {
    let idx = read_expr(lexer);
    lexer.expect_skip("]");
    AST::UnaryOp(Rc::new(AST::BinaryOp(Rc::new(ast), Rc::new(idx), node::CBinOps::Add)),
                 node::CUnaryOps::Deref)
}

fn read_field(lexer: &mut Lexer, ast: AST) -> AST {
    let field = lexer.get_e();
    if field.kind != TokenKind::Identifier {
        error::error_exit(lexer.cur_line, "expected field name");
    }

    let field_name = field.val;
    AST::StructRef(Rc::new(ast), field_name)
}

fn read_primary(lexer: &mut Lexer) -> AST {
    let tok = lexer
        .get()
        .or_else(|| error::error_exit(lexer.cur_line, "expected primary"))
        .unwrap();
    match tok.kind {
        TokenKind::IntNumber => {
            let num_literal = tok.val.clone();
            if num_literal.len() > 2 && num_literal.chars().nth(1).unwrap() == 'x' {
                AST::Int(read_hex_num(&num_literal[2..]))
            } else {
                AST::Int(read_dec_num(num_literal.as_str()))
            }
        }
        // TokenKind::FloatNumber => None,
        TokenKind::Identifier => AST::Variable(tok.val),
        // TokenKind::String => None,
        // TokenKind::Char => None,
        TokenKind::Symbol => {
            match tok.val.as_str() {
                "(" => {
                    let expr = read_expr(lexer);
                    lexer.skip(")");
                    expr
                }
                _ => {
                    error::error_exit(lexer.cur_line,
                                      format!("read_primary unknown symbol '{}'", tok.val.as_str())
                                          .as_str())
                }
            }
        }
        // TokenKind::Newline => None,
        _ => {
            error::error_exit(lexer.cur_line,
                              format!("read_primary unknown token {:?}", tok.kind).as_str())
        }
    }
}
fn read_dec_num(num_literal: &str) -> i32 {
    let mut n = 0;
    for c in num_literal.chars() {
        match c {
            '0'...'9' => n = n * 10 + c.to_digit(10).unwrap() as i32, 
            _ => {} // TODO: suffix
        }
    }
    n
}
fn read_hex_num(num_literal: &str) -> i32 {
    let mut n = 0;
    for c in num_literal[2..].chars() {
        match c {
            '0'...'9' | 'A'...'F' | 'a'...'f' => n = n * 16 + c.to_digit(16).unwrap() as i32, 
            _ => {} // TODO: suffix
        }
    }
    n
}
////////// operators end here
