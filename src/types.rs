use std::rc::Rc;

#[derive(PartialEq, Debug, Clone)]
pub enum Sign {
    Signed,
    Unsigned,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Type {
    Void,
    Char(Sign), // sign(false is signed)
    Short(Sign), // sign
    Int(Sign), // sign
    Long(Sign), // sign
    LLong(Sign), // sign
    Float,
    Double,
    Ptr(Rc<Type>),
    Array(Rc<Type>, usize), // ary type, size
    Func(Rc<Type>, Vec<Type>), // return type, param types
}
