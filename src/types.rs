use std::rc::Rc;
use node::{AST, ASTKind};

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
    pub fn get_elem_ty(&self) -> Option<Type> {
        match self {
            &Type::Ptr(ref elem_ty) |
            &Type::Array(ref elem_ty, _) => Some((**elem_ty).clone()),
            _ => None,
        }
    }
    pub fn get_return_ty(&self) -> Option<Type> {
        match self {
            &Type::Func(ref ret_ty, _, _) => Some((**ret_ty).clone()),
            _ => None,
        }
    }
    pub fn get_field_ty(&self, field_name: String) -> Option<Type> {
        match self {
            &Type::Struct(_, ref fields) |
            &Type::Union(_, ref fields, _) => {
                for field in fields {
                    if let ASTKind::VariableDecl(ref ty, ref name, _, _) = field.kind {
                        if *name == field_name {
                            return Some((*ty).clone());
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
    pub fn get_name(&self) -> Option<String> {
        match self {
            &Type::Struct(ref name, _) |
            &Type::Union(ref name, _, _) => Some(name.to_owned()),
            _ => None,
        }
    }

    pub fn calc_size(&self) -> usize {
        match self {
            &Type::Void => 0,
            &Type::Char(_) => 1,
            &Type::Short(_) => 2,
            &Type::Int(_) => 4,
            &Type::Long(_) => 8,
            &Type::LLong(_) => 8,
            &Type::Float => 4,
            &Type::Double => 8,
            &Type::Ptr(ref _elemty) => 8,
            &Type::Array(ref elemty, ref size) => (*size * elemty.calc_size() as i32) as usize,
            &Type::Func(ref _ret_type, ref _param_types, ref _is_vararg) => 1,
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
                    size_total += if let ASTKind::VariableDecl(ref ty, _, _, _) = field.kind {
                        let size = ty.calc_size();
                        size + calc_padding(size_total, size)
                    } else {
                        0
                    };
                }
                size_total
            }
            &Type::Union(ref _name, ref fields, ref max_nth) => {
                if let ASTKind::VariableDecl(ref ty, _, _, _) = fields[*max_nth].kind {
                    ty.calc_size()
                } else {
                    0
                }
            }
            &Type::Enum => 4,
        }
    }
}
