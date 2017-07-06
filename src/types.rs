use std::rc::Rc;
use node::AST;

#[derive(PartialEq, Debug, Clone, Hash)]
pub enum Sign {
    Signed,
    Unsigned,
}

#[derive(PartialEq, Debug, Clone)]
pub enum StorageClass {
    Typedef,
    Extern,
    Static,
    Auto,
    Register,
}


#[derive(Debug, Clone)]
pub enum Type {
    Void,
    Char(Sign),
    Short(Sign),
    Int(Sign),
    Long(Sign),
    LLong(Sign),
    Float,
    Double,
    Ptr(Rc<Type>),
    Array(Rc<Type>, i32), // ary elem type, size
    Func(Rc<Type>, Vec<Type>, bool), // return type, param types, vararg
    Struct(String, Vec<AST>), // name, fields
    Union(String, Vec<AST>, usize), // name, fields, means size of nth field is size of the union
    Enum, // as same as Int
}

impl Type {
    pub fn get_ptr_elem_ty(&self) -> Option<Type> {
        if let &Type::Ptr(ref elem_ty) = self {
            Some((**elem_ty).clone())
        } else {
            None
        }
    }

    pub fn calc_size(&self) -> usize {
        match self {
            &Type::Void => 0,
            &Type::Char(_) => 1,
            &Type::Short(_) => 2,
            &Type::Int(_) => 4,
            &Type::Long(_) => 4,
            &Type::LLong(_) => 8,
            &Type::Float => 4,
            &Type::Double => 8,
            &Type::Ptr(ref _elemty) => 8,
            &Type::Array(ref elemty, ref size) => (*size * elemty.calc_size() as i32) as usize,
            &Type::Func(ref _ret_type, ref _param_types, ref _is_vararg) => 8,
            // TODO: must fix this sloppy implementation
            &Type::Struct(ref _name, ref fields) => {
                let mut size_total = 0;
                let calc_padding = |off, align| -> usize {
                    if off % align == 0 {
                        0
                    } else {
                        align - off % align
                    }
                };
                for field in fields {
                    size_total += if let &AST::VariableDecl(ref ty, _, _, _) = field {
                        let size = ty.calc_size();
                        size + calc_padding(size_total, size)
                    } else {
                        0
                    };
                }
                size_total
            }
            &Type::Union(ref _name, ref fields, ref max_nth) => {
                if let &AST::VariableDecl(ref ty, _, _, _) = &fields[*max_nth] {
                    ty.calc_size()
                } else {
                    0
                }
            }
            &Type::Enum => 4,
        }
    }
}
