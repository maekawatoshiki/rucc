use lexer;
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
                // if t.kind == TokenKind::Newline {
                //     println!();
                // } else {
                println!("t:{}{}", if t.space { " " } else { "" }, t.val);
                // }
            }
            None => break,
        }
    }

    // lexer = Lexer::new(filename.to_string(), s.as_str());
    // nodes.push(read_toplevel(&mut lexer));
    // nodes.pop().unwrap().show();

    nodes
}

pub fn run(input: String) -> Vec<AST> {
    let mut nodes: Vec<AST> = Vec::new();
    let mut lexer = Lexer::new("__input__".to_string(), input.as_str());

    nodes.push(read_toplevel(&mut lexer));

    nodes
}

fn read_toplevel(lexer: &mut Lexer) -> AST {
    if is_function_def(lexer) {
        println!("this is function");
        read_func_def(lexer)
    } else {
        read_expr(lexer)
    }
}

fn read_func_def(lexer: &mut Lexer) -> AST {
    read_decl(lexer);
    AST::Int(0)
}

fn is_function_def(lexer: &mut Lexer) -> bool {
    let mut buf: Vec<Token> = Vec::new();
    let mut is_funcdef = false;

    loop {
        let mut tok = lexer.get().unwrap();
        buf.push(tok.clone());

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

        if tok.val == "(" {
            continue;
        }

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
        return true;
    }
    match token.val.as_str() {
        "int" => true,
        _ => false,
    }
}

fn read_decl(lexer: &mut Lexer) -> AST {
    let basety = read_type_spec(lexer);
    println!("read_decl: baesty: {:?}", basety);
    AST::Int(0)
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

        if tok.kind != TokenKind::Identifier {
            lexer.unget(tok);
            break;
        }

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
            _ => {}
        }
    }

    // if sign is not expected
    if sign.is_none() {
        sign = Some(Sign::Signed);
    }

    let mut ty: Option<Type> = None;
    // kind is None => 'signed' or 'unsigned'
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
    let mut lhs = read_logor(lexer);
    while lexer.skip(",") {
        let rhs = read_logor(lexer);
        lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Comma);
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
            "*" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Indir),
            "&" => return AST::UnaryOp(Rc::new(read_cast(lexer)), node::CUnaryOps::Addr),
            _ => {}
        }
    }
    lexer.unget(tok);
    read_postfix(lexer)
}
fn read_postfix(lexer: &mut Lexer) -> AST {
    read_primary(lexer)
}

fn read_primary(lexer: &mut Lexer) -> AST {
    let tok = lexer
        .get()
        .or_else(|| error::error_exit(lexer.cur_line, "expected primary"))
        .unwrap();
    match tok.kind {
        // TokenKind::Identifier => None,
        TokenKind::IntNumber => {
            let a = tok.val.clone();
            if a.len() > 2 && a.chars().nth(1).unwrap() == 'x' {
                AST::Int(u32::from_str_radix(&a[2..], 16).unwrap() as i32)
            } else {
                let mut n = 0;
                for c in a.chars() {
                    match c {
                        '0'...'9' => n = n * 10 + c.to_digit(10).unwrap() as i32, 
                        _ => {} // TODO: suffix
                    }
                }
                AST::Int(n)
            }
        }
        // TokenKind::FloatNumber => None,
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
////////// operators end here
