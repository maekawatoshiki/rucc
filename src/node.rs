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
    Return(Option<Rc<AST>>),
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
        self.eval()
    }

    fn eval(&self) -> i32 {
        match self {
            &AST::Int(n) => n,
            &AST::UnaryOp(ref e, CUnaryOps::LNot) => (e.eval() == 0) as i32,
            &AST::UnaryOp(ref e, CUnaryOps::BNot) => !e.eval(),
            &AST::UnaryOp(ref e, CUnaryOps::Plus) => e.eval(),
            &AST::UnaryOp(ref e, CUnaryOps::Minus) => -e.eval(),
            &AST::UnaryOp(ref e, CUnaryOps::Inc) => e.eval() + 1,
            &AST::UnaryOp(ref e, CUnaryOps::Dec) => e.eval() - 1,
            &AST::UnaryOp(ref e, CUnaryOps::Deref) => e.eval(),
            &AST::UnaryOp(ref e, CUnaryOps::Addr) => e.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Add) => l.eval() + r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Sub) => l.eval() - r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Mul) => l.eval() * r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Div) => l.eval() / r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Rem) => l.eval() % r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::And) => l.eval() & r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Or) => l.eval() | r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Xor) => l.eval() ^ r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::LAnd) => l.eval() & r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::LOr) => l.eval() | r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Eq) => (l.eval() == r.eval()) as i32,
            &AST::BinaryOp(ref l, ref r, CBinOps::Ne) => (l.eval() != r.eval()) as i32,
            &AST::BinaryOp(ref l, ref r, CBinOps::Lt) => (l.eval() < r.eval()) as i32,
            &AST::BinaryOp(ref l, ref r, CBinOps::Gt) => (l.eval() > r.eval()) as i32,
            &AST::BinaryOp(ref l, ref r, CBinOps::Le) => (l.eval() <= r.eval()) as i32,
            &AST::BinaryOp(ref l, ref r, CBinOps::Ge) => (l.eval() >= r.eval()) as i32,
            &AST::BinaryOp(ref l, ref r, CBinOps::Shl) => l.eval() << r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Shr) => l.eval() >> r.eval(),
            &AST::BinaryOp(ref l, ref r, CBinOps::Comma) => {
                l.eval();
                r.eval()
            }
            &AST::BinaryOp(ref l, ref r, _) => {
                l.eval();
                r.eval();
                0
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
                if retval.is_some() {
                    retval.clone().unwrap().show();
                }
                print!(")");
            }
        };
    }
}
