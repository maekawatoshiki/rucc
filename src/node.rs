use std::rc::Rc;
use types::Type;

#[derive( Debug)]
pub enum AST {
    Int(i32),
    Float(f64),
    Char(i32),
    String(String),
    Variable(String),
    VariableDecl(Type, String, Option<Rc<AST>>), // type, name, init val
    UnaryOp(Rc<AST>, CUnaryOps),
    BinaryOp(Rc<AST>, Rc<AST>, CBinOps),
    FuncDef(Type, Vec<String>, String, Rc<AST>), // functype, param names, func name, body
    Block(Vec<AST>),
    FuncCall(Rc<AST>, Vec<AST>),
    StructRef(Rc<AST>, String), // String is name of struct field
    Return(Rc<AST>),
}

#[derive(Debug)]
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
    Comma,
    Assign,
}

#[derive(Debug)]
pub enum CUnaryOps {
    LNot,
    BNot,
    Plus,
    Minus,
    Inc,
    Dec,
    Deref,
    Addr,
    // TODO: add Cast, Sizeof
}

impl AST {
    pub fn eval_constexpr(&self) -> i32 {
        match self {
            &AST::Int(n) => n,
            &AST::UnaryOp(ref aexpr, ref op) => {
                let expr = aexpr.eval_constexpr();
                match op {
                    &CUnaryOps::LNot => (expr == 0) as i32,
                    &CUnaryOps::BNot => !expr,
                    &CUnaryOps::Plus => expr,
                    &CUnaryOps::Minus => -expr,
                    &CUnaryOps::Inc => 1 + expr,
                    &CUnaryOps::Dec => expr - 1,
                    &CUnaryOps::Deref => expr,
                    &CUnaryOps::Addr => expr,
                }
            }
            &AST::BinaryOp(ref alhs, ref arhs, ref op) => {
                let lhs = alhs.eval_constexpr();
                let rhs = arhs.eval_constexpr();
                match op {
                    &CBinOps::Add => lhs + rhs,
                    &CBinOps::Sub => lhs - rhs,
                    &CBinOps::Mul => lhs * rhs,
                    &CBinOps::Div => lhs / rhs,
                    &CBinOps::Rem => lhs % rhs,
                    &CBinOps::And => lhs & rhs,
                    &CBinOps::Or => lhs | rhs,
                    &CBinOps::Xor => lhs ^ rhs,
                    &CBinOps::LAnd => lhs & rhs,
                    &CBinOps::LOr => lhs | rhs,
                    &CBinOps::Eq => (lhs == rhs) as i32,
                    &CBinOps::Ne => (lhs != rhs) as i32,
                    &CBinOps::Lt => (lhs < rhs) as i32,
                    &CBinOps::Gt => (lhs > rhs) as i32,
                    &CBinOps::Le => (lhs <= rhs) as i32,
                    &CBinOps::Ge => (lhs >= rhs) as i32,
                    &CBinOps::Shl => lhs << rhs,
                    &CBinOps::Shr => lhs >> rhs,
                    &CBinOps::Comma => rhs,
                    _ => 0,
                }
            }
            _ => 0,
        }
    }
    pub fn show(&self) {
        match self {
            &AST::Int(n) => print!("{} ", n),
            &AST::Float(n) => print!("{} ", n),
            &AST::Char(c) => print!("'{}' ", c),
            &AST::String(ref s) => print!("\"{}\" ", s),
            &AST::Variable(ref name) => print!("{} ", name),
            &AST::VariableDecl(ref ty, ref name, ref init) => {
                print!("(var-decl {:?} {}", ty, name);
                if init.is_some() {
                    print!(" (init ");
                    init.clone().unwrap().show();
                    print!(")");
                }
                print!(")");
            }
            &AST::UnaryOp(ref expr, ref op) => {
                print!("({:?} ", op);
                expr.show();
                print!(")");
            }
            &AST::BinaryOp(ref lhs, ref rhs, ref op) => {
                print!("({:?} ", op);
                lhs.show();
                rhs.show();
                print!(")");
            }
            &AST::FuncDef(ref functy, ref param_names, ref name, ref body) => {
                print!("(def-func {} {:?} {:?}", name, functy, param_names);
                body.show();
                print!(")");
            }
            &AST::Block(ref body) => {
                for stmt in body {
                    stmt.show();
                }
            }
            &AST::FuncCall(ref f, ref args) => {
                print!("(func-call ");
                f.show();
                print!(" ");
                for arg in args {
                    arg.show();
                }
                print!(")");
            }
            &AST::StructRef(ref s, ref field) => {
                print!("(struct-ref ");
                s.show();
                print!(" {})", field);
            }
            &AST::Return(ref retval) => {
                print!("(return ");
                retval.show();
                print!(")");
            }
        };
    }
}
