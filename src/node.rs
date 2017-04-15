// use std::io;
// use std::io::prelude::*;
// use std::iter;
// use std::str;
use std::rc::Rc;

pub enum AST {
    Int(i32),
    Float(f64),
    Variable(String),
    BinaryOp(BinaryOpAST),
}

pub enum CBinOps {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    LAnd,
    LOr,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Shl,
    Shr,
}

pub struct BinaryOpAST {
    pub lhs: Rc<AST>,
    pub rhs: Rc<AST>,
    pub op: CBinOps,
}
impl BinaryOpAST {
    pub fn new(lhs: Rc<AST>, rhs: Rc<AST>, op: String) -> BinaryOpAST {
        let cop = match op.as_str() {
            "+" => CBinOps::Add,
            "-" => CBinOps::Sub,
            "*" => CBinOps::Mul,
            "/" => CBinOps::Div,
            "%" => CBinOps::Rem,
            "&" => CBinOps::And,
            "|" => CBinOps::Or,
            "^" => CBinOps::Xor,
            "&&" => CBinOps::LAnd,
            "||" => CBinOps::LOr,
            "==" => CBinOps::Eq,
            "!=" => CBinOps::Ne,
            "<" => CBinOps::Lt,
            ">" => CBinOps::Gt,
            "<=" => CBinOps::Le,
            ">=" => CBinOps::Ge,
            "<<" => CBinOps::Shl,
            ">>" => CBinOps::Shr,
            _ => CBinOps::Add,
        };

        BinaryOpAST {
            lhs: lhs,
            rhs: rhs,
            op: CBinOps::Add,
        }
    }
    pub fn eval_constexpr(&self) -> i32 {
        let lhs = self.lhs.eval_constexpr();
        let rhs = self.rhs.eval_constexpr();
        match self.op {
            CBinOps::Add => lhs + rhs,
            CBinOps::Sub => lhs - rhs,
            CBinOps::Mul => lhs * rhs,
            CBinOps::Div => lhs / rhs,
            CBinOps::Rem => lhs % rhs,
            CBinOps::And => lhs & rhs,
            CBinOps::Or => lhs | rhs,
            CBinOps::Xor => lhs ^ rhs,
            CBinOps::LAnd => lhs & rhs,
            CBinOps::LOr => lhs | rhs,
            CBinOps::Eq => (lhs == rhs) as i32,
            CBinOps::Ne => (lhs != rhs) as i32,
            CBinOps::Lt => (lhs < rhs) as i32,
            CBinOps::Gt => (lhs > rhs) as i32,
            CBinOps::Le => (lhs <= rhs) as i32,
            CBinOps::Ge => (lhs >= rhs) as i32,
            CBinOps::Shl => lhs << rhs,
            CBinOps::Shr => lhs >> rhs,
        }
    }
}

impl AST {
    pub fn eval_constexpr(&self) -> i32 {
        match self {
            &AST::Int(n) => n,
            &AST::BinaryOp(ref bin) => bin.eval_constexpr(),
            _ => 0,
        }
    }
}
