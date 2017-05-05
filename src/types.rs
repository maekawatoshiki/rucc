use std::rc::Rc;

#[derive(PartialEq, Debug, Clone)]
pub enum Sign {
    Signed,
    Unsigned,
}

#[derive(PartialEq, Debug, Clone)]
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
}
