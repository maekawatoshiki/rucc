use lexer::{Lexer, TokenKind};
use node::AST;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::{str, u32};
use std::rc::Rc;

use node;
use error;

pub fn run_file(filename: String) -> Vec<AST> {
    let nodes: Vec<AST> = Vec::new();
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
    nodes
}

pub fn run(input: String) -> Vec<AST> {
    let mut nodes: Vec<AST> = Vec::new();
    let mut lexer = Lexer::new("__input__".to_string(), input.as_str());

    nodes.push(read_toplevel(&mut lexer));

    nodes
}

fn read_toplevel(lexer: &mut Lexer) -> AST {
    read_expr(lexer)
}

////////// operators start here
pub fn read_expr(lexer: &mut Lexer) -> AST {
    let lhs = read_comma(lexer);
    lhs
}
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
