use lexer::{Lexer, Token, TokenKind};
use node::AST;
use node;
use error;
// use std::io::{BufRead, BufReader};
use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::iter;
use std::str;
use std::collections::VecDeque;
use std::path;
use std::process;
use std::collections::{HashMap, HashSet};
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

fn read_expr(lexer: &mut Lexer) -> AST {
    let mut lhs = read_comma(lexer);
    lhs
}

fn read_comma(lexer: &mut Lexer) -> AST {
    let mut lhs = read_add_sub(lexer);
    while lexer.skip(",") {
        let rhs = read_add_sub(lexer);
        lhs =
            AST::BinaryOp(node::BinaryOpAST::new(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Comma));
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

fn read_primary(lexer: &mut Lexer) -> AST {
    match lexer.get() {
        Some(tok) => {
            match tok.kind {
                // TokenKind::Identifier => None,
                TokenKind::IntNumber => AST::Int(tok.val.parse::<i32>().unwrap()),
                // TokenKind::FloatNumber => None,
                // TokenKind::String => None,
                // TokenKind::Char => None,
                // TokenKind::Symbol => None,
                // TokenKind::Newline => None,
                _ => error::error_exit(1, "AA"),
            }
        }
        None => error::error_exit(1, "AA"),
    }
}
