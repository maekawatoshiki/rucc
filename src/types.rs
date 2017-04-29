use std::rc::Rc;

#[derive(PartialEq, Debug, Clone)]
pub enum Type {
    Void,
    Char(bool), // sign
    Short(bool), // sign
    Int(bool), // sign
    Long(bool), // sign
    LLong(bool), // sign
    Ptr(Rc<Type>),
    Array(Rc<Type>, usize), // ary type, size
    Func(Rc<Type>, Vec<Type>), // return type, param types
}
