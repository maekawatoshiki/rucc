use lexer::{Lexer, Token, TokenKind};
use node::AST;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::iter;
use std::str;
use std::collections::VecDeque;
use std::rc::Rc;

use node;
use error;

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
                println!("token:{}{}", if t.space { " " } else { "" }, t.val);
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
    // loop {
    //     let tok = lexer.get();
    //     match tok {
    //         Some(t) => {
    //             println!("token:{}{}", if t.space { " " } else { "" }, t.val);
    //         }
    //         None => break,
    //     }
    // }
    nodes
}

fn read_toplevel(lexer: &mut Lexer) -> AST {
    read_expr(lexer)
}

////////// operators start here
fn read_expr(lexer: &mut Lexer) -> AST {
    let mut lhs = read_comma(lexer);
    lhs
}
fn read_comma(lexer: &mut Lexer) -> AST {
    let mut lhs = read_logor(lexer);
    while lexer.skip(",") {
        let rhs = read_logor(lexer);
        lhs =
            AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Comma));
    }
    lhs
}
fn read_logor(lexer: &mut Lexer) -> AST {
    let mut lhs = read_logand(lexer);
    while lexer.skip("||") {
        let rhs = read_logand(lexer);
        lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LOr));
    }
    lhs
}
fn read_logand(lexer: &mut Lexer) -> AST {
    let mut lhs = read_or(lexer);
    while lexer.skip("&&") {
        let rhs = read_or(lexer);
        lhs =
            AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LAnd));
    }
    lhs
}
fn read_or(lexer: &mut Lexer) -> AST {
    let mut lhs = read_xor(lexer);
    while lexer.skip("|") {
        let rhs = read_xor(lexer);
        lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Or));
    }
    lhs
}
fn read_xor(lexer: &mut Lexer) -> AST {
    let mut lhs = read_and(lexer);
    while lexer.skip("^") {
        let rhs = read_and(lexer);
        lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Xor));
    }
    lhs
}
fn read_and(lexer: &mut Lexer) -> AST {
    let mut lhs = read_eq_ne(lexer);
    while lexer.skip("&") {
        let rhs = read_eq_ne(lexer);
        lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs), Rc::new(rhs), node::CBinOps::And));
    }
    lhs
}
fn read_eq_ne(lexer: &mut Lexer) -> AST {
    let mut lhs = read_relation(lexer);
    loop {
        if lexer.skip("==") {
            let rhs = read_relation(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Eq));
        } else if lexer.skip("!=") {
            let rhs = read_relation(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Ne));
        } else {
            break;
        }
    }
    lhs
}
fn read_relation(lexer: &mut Lexer) -> AST {
    let mut lhs = read_add_sub(lexer);
    loop {
        if lexer.skip("<") {
            let rhs = read_add_sub(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Lt));
        } else if lexer.skip("<=") {
            let rhs = read_add_sub(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Le));
        } else if lexer.skip(">") {
            let rhs = read_add_sub(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Gt));
        } else if lexer.skip(">=") {
            let rhs = read_add_sub(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Ge));
        } else {
            break;
        }
    }
    lhs
}
fn read_add_sub(lexer: &mut Lexer) -> AST {
    let mut lhs = read_primary(lexer);
    loop {
        if lexer.skip("+") {
            let rhs = read_primary(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Add));
        } else if lexer.skip("-") {
            let rhs = read_primary(lexer);
            lhs = AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs),
                                                       Rc::new(rhs),
                                                       node::CBinOps::Sub));
        } else {
            break;
        }
    }
    lhs
}
////////// operators end here

fn read_primary(lexer: &mut Lexer) -> AST {
    match lexer.get() {
        Some(tok) => {
            match tok.kind {
                // TokenKind::Identifier => None,
                TokenKind::IntNumber => AST::Int(tok.val.parse::<i32>().unwrap()),
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
                        _ => error::error_exit(lexer.cur_line, "err"),
                    }
                }
                // TokenKind::Newline => None,
                _ => error::error_exit(lexer.cur_line, "err"),
            }
        }
        _ => error::error_exit(lexer.cur_line, "err"),
    }
}
