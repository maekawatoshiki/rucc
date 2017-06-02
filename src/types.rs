use std::rc::Rc;
use node::AST;

#[derive(PartialEq, Debug, Clone, Hash)]
pub enum Sign {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone)]
pub enum Type {
    Void,
    Char(Sign), // sign
    Short(Sign), // sign
    Int(Sign), // sign
    Long(Sign), // sign
    LLong(Sign), // sign
    Float,
    Double,
    Ptr(Rc<Type>),
    Array(Rc<Type>, i32), // ary type, size
    Func(Rc<Type>, Vec<Type>, bool), // return type, param types, vararg
    Struct(String, Vec<AST>),
}

impl Type {
    pub fn get_ptr_elem_ty(&self) -> Option<Type> {
        if let &Type::Ptr(ref elem_ty) = self {
            Some((**elem_ty).clone())
        } else {
            None
        }
    }
}
