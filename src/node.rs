use lexer::Pos;
use parser::{Error, ParseR};
use std::boxed::Box;
use std::marker::Send;
use types::{StorageClass, Type};

#[derive(Debug, Clone)]
pub struct AST {
    pub kind: ASTKind,
    pub pos: Pos,
}

impl AST {
    pub fn new(kind: ASTKind, pos: Pos) -> AST {
        AST {
            kind: kind,
            pos: pos,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Bits {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
}

#[derive(Debug, Clone)]
pub enum ASTKind {
    Int(i64, Bits),
    Float(f64),
    Char(i32),
    String(String),
    Typedef(Type, String), // from, to ( typedef from to; )
    TypeCast(Box<AST>, Type),
    Load(Box<AST>),
    Variable(Type, String),
    VariableDecl(Type, String, StorageClass, Option<Box<AST>>), // type, name, init val
    ConstArray(Vec<AST>),
    ConstStruct(Vec<AST>),
    UnaryOp(Box<AST>, CUnaryOps),
    BinaryOp(Box<AST>, Box<AST>, CBinOps),
    TernaryOp(Box<AST>, Box<AST>, Box<AST>), // cond then else
    FuncDef(Type, Vec<String>, String, Box<AST>), // functype, param names, func name, body
    Block(Vec<AST>),
    Compound(Vec<AST>),
    If(Box<AST>, Box<AST>, Box<AST>), // cond, then stmt, else stmt
    For(Box<AST>, Box<AST>, Box<AST>, Box<AST>), // init, cond, step, body
    While(Box<AST>, Box<AST>),        // cond, body
    DoWhile(Box<AST>, Box<AST>),      // cond, body
    Switch(Box<AST>, Box<AST>),       // cond, stmt
    Case(Box<AST>),
    DefaultL,
    Goto(String),  // label name
    Label(String), // label name
    FuncCall(Box<AST>, Vec<AST>),
    StructRef(Box<AST>, String), // String is name of struct field
    Break,
    Continue,
    Return(Option<Box<AST>>),
}

unsafe impl Send for AST {}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum CUnaryOps {
    LNot,
    BNot,
    Minus,
    // TODO: Inc and Dec is actually POSTFIX.
    Inc,
    Dec,
    Deref,
    Addr,
    Sizeof,
    // TODO: add Cast, Sizeof
}

impl AST {
    pub fn eval_constexpr(&self) -> ParseR<i64> {
        self.eval()
    }

    fn eval(&self) -> ParseR<i64> {
        Ok(match self.kind {
            ASTKind::Int(n, _) => n,
            ASTKind::TypeCast(ref e, _) => e.eval()?,
            ASTKind::UnaryOp(ref e, CUnaryOps::LNot) => (e.eval()? == 0) as i64,
            ASTKind::UnaryOp(ref e, CUnaryOps::BNot) => !e.eval()?,
            ASTKind::UnaryOp(ref e, CUnaryOps::Minus) => -e.eval()?,
            ASTKind::UnaryOp(ref e, CUnaryOps::Inc) => e.eval()? + 1,
            ASTKind::UnaryOp(ref e, CUnaryOps::Dec) => e.eval()? - 1,
            ASTKind::UnaryOp(ref e, CUnaryOps::Deref) => e.eval()?,
            ASTKind::UnaryOp(ref e, CUnaryOps::Addr) => e.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Add) => l.eval()? + r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Sub) => l.eval()? - r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Mul) => l.eval()? * r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Div) => l.eval()? / r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Rem) => l.eval()? % r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::And) => l.eval()? & r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Or) => l.eval()? | r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Xor) => l.eval()? ^ r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::LAnd) => l.eval()? & r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::LOr) => l.eval()? | r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Eq) => (l.eval()? == r.eval()?) as i64,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Ne) => (l.eval()? != r.eval()?) as i64,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Lt) => (l.eval()? < r.eval()?) as i64,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Gt) => (l.eval()? > r.eval()?) as i64,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Le) => (l.eval()? <= r.eval()?) as i64,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Ge) => (l.eval()? >= r.eval()?) as i64,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Shl) => l.eval()? << r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Shr) => l.eval()? >> r.eval()?,
            ASTKind::BinaryOp(ref l, ref r, CBinOps::Comma) => {
                l.eval()?;
                r.eval()?
            }
            ASTKind::BinaryOp(ref l, ref r, _) => {
                l.eval()?;
                r.eval()?;
                0
            }
            ASTKind::TernaryOp(ref cond, ref l, ref r) => {
                if cond.eval()? != 0 {
                    l.eval()?
                } else {
                    r.eval()?
                }
            }
            _ => return Err(Error::Something),
        })
    }

    pub fn is_const(&self) -> bool {
        match self.kind {
            ASTKind::Int(_, _) | ASTKind::Float(_) | ASTKind::String(_) | ASTKind::Char(_) => true,
            // String(String),
            _ => false,
        }
    }

    pub fn get_variable_name<'a>(&'a self) -> Option<&'a str> {
        if let ASTKind::Variable(_, ref name) = self.kind {
            return Some(name.as_str());
        }
        None
    }

    pub fn show(&self) {
        match self.kind {
            ASTKind::Int(n, _) => print!("{} ", n),
            ASTKind::Float(n) => print!("{} ", n),
            ASTKind::Char(c) => print!("'{}' ", c),
            ASTKind::String(ref s) => print!("\"{}\" ", s),
            ASTKind::Typedef(ref a, ref b) => print!("(typedef {:?} {})", a, b),
            ASTKind::TypeCast(ref e, ref t) => {
                print!("(typecast {:?} ", t);
                e.show();
                print!(")");
            }
            ASTKind::Load(ref expr) => {
                print!("(load ");
                expr.show();
                print!(")");
            }
            ASTKind::Variable(ref ty, ref name) => print!("({:?} {}) ", ty, name),
            ASTKind::VariableDecl(ref ty, ref name, ref sclass, ref init) => {
                print!("(var-decl {:?} {:?} {}", ty, sclass, name);
                if init.is_some() {
                    print!(" (init ");
                    init.clone().unwrap().show();
                    print!(")");
                }
                print!(")");
            }
            ASTKind::ConstArray(ref elems) => {
                print!("(const-array ");
                for elem in elems {
                    elem.show();
                }
                print!(")");
            }
            ASTKind::ConstStruct(ref elems) => {
                print!("(const-struct ");
                for elem in elems {
                    elem.show();
                }
                print!(")");
            }
            ASTKind::UnaryOp(ref expr, ref op) => {
                print!("({:?} ", op);
                expr.show();
                print!(")");
            }
            ASTKind::BinaryOp(ref lhs, ref rhs, ref op) => {
                print!("({:?} ", op);
                lhs.show();
                rhs.show();
                print!(")");
            }
            ASTKind::TernaryOp(ref cond, ref lhs, ref rhs) => {
                print!("(?: ");
                cond.show();
                print!(" ");
                lhs.show();
                print!(" ");
                rhs.show();
                print!(")");
            }
            ASTKind::FuncDef(ref functy, ref param_names, ref name, ref body) => {
                print!("(def-func {} {:?} {:?}", name, functy, param_names);
                body.show();
                print!(")");
            }
            ASTKind::Block(ref body) => {
                for stmt in body {
                    stmt.show();
                }
            }
            ASTKind::Compound(ref body) => {
                for stmt in body {
                    stmt.show();
                }
            }
            ASTKind::If(ref cond, ref then_b, ref else_b) => {
                print!("(if ");
                cond.show();
                print!("(");
                then_b.clone().show();
                print!(")(");
                else_b.clone().show();
                print!("))");
            }
            ASTKind::For(ref init, ref cond, ref step, ref body) => {
                print!("(for ");
                init.show();
                print!("; ");
                cond.show();
                print!("; ");
                step.show();
                print!(" (");
                body.show();
                print!(")");
            }
            ASTKind::DoWhile(ref cond, ref body) => {
                print!("(do-while ");
                cond.show();
                print!("(");
                body.show();
                print!("))");
            }
            ASTKind::While(ref cond, ref body) => {
                print!("(while ");
                cond.show();
                print!("(");
                body.show();
                print!("))");
            }
            ASTKind::Switch(ref cond, ref body) => {
                print!("(switch ");
                cond.show();
                print!("(");
                body.show();
                print!("))");
            }
            ASTKind::Case(ref expr) => {
                print!("(case ");
                expr.show();
                print!(")");
            }
            ASTKind::DefaultL => {
                print!("(default)");
            }
            ASTKind::Goto(ref label_name) => {
                print!("(goto {})", label_name);
            }
            ASTKind::Label(ref name) => {
                print!("(label {})", name);
            }
            ASTKind::FuncCall(ref f, ref args) => {
                print!("(func-call ");
                f.show();
                print!(" ");
                for arg in args {
                    arg.show();
                }
                print!(")");
            }
            ASTKind::StructRef(ref s, ref field) => {
                print!("(struct-ref ");
                s.show();
                print!(" {})", field);
            }
            ASTKind::Continue => {
                print!("(continue)");
            }
            ASTKind::Break => {
                print!("(break)");
            }
            ASTKind::Return(ref retval) => {
                print!("(return ");
                if retval.is_some() {
                    retval.clone().unwrap().show();
                }
                print!(")");
            }
        };
    }
}
