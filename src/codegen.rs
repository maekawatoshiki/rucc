extern crate llvm_sys as llvm;

use std;
use std::ffi::CString;
use std::ptr;
use std::rc::Rc;
use std::collections::HashMap;

use self::llvm::core::*;
use self::llvm::prelude::*;

use node;
use types::{Type, StorageClass, Sign};
use error;

// used by global_varmap and local_varmap(not to use tuples)
#[derive(Clone)]
struct VarInfo {
    ty: Type,
    llvm_ty: LLVMTypeRef,
    llvm_val: LLVMValueRef,
}
impl VarInfo {
    fn new(ty: Type, llvm_ty: LLVMTypeRef, llvm_val: LLVMValueRef) -> VarInfo {
        VarInfo {
            ty: ty,
            llvm_ty: llvm_ty,
            llvm_val: llvm_val,
        }
    }
}

struct RectypeInfo {
    field_pos: HashMap<String, u32>,
    field_types: Vec<Type>,
    field_llvm_types: Vec<LLVMTypeRef>,
    llvm_rectype: LLVMTypeRef,
    is_struct: bool,
}
impl RectypeInfo {
    fn new(field_pos: HashMap<String, u32>,
           field_types: Vec<Type>,
           field_llvm_types: Vec<LLVMTypeRef>,
           llvm_rectype: LLVMTypeRef,
           is_struct: bool)
           -> RectypeInfo {
        RectypeInfo {
            field_pos: field_pos,
            field_types: field_types,
            field_llvm_types: field_llvm_types,
            llvm_rectype: llvm_rectype,
            is_struct: is_struct,
        }
    }
}

pub enum Error {
    MsgWithLine(String, i32),
    Msg(String),
}

type CodegenResult = Result<(LLVMValueRef, Option<Type>), Error>;

pub struct Codegen {
    context: LLVMContextRef,
    module: LLVMModuleRef,
    builder: LLVMBuilderRef,
    global_varmap: HashMap<String, VarInfo>,
    local_varmap: Vec<HashMap<String, VarInfo>>,
    llvm_struct_map: HashMap<String, RectypeInfo>,
    _data_layout: *const i8,
    cur_func: Option<LLVMValueRef>,
}

impl Codegen {
    pub unsafe fn new(mod_name: &str) -> Codegen {
        let c_mod_name = CString::new(mod_name).unwrap();
        let module = LLVMModuleCreateWithNameInContext(c_mod_name.as_ptr(), LLVMContextCreate());
        let mut global_varmap = HashMap::new();

        let llvm_memcpy_ty = Type::Func(Rc::new(Type::Void),
                                        vec![Type::Ptr(Rc::new(Type::Char(Sign::Signed))),
                                             Type::Ptr(Rc::new(Type::Char(Sign::Signed))),
                                             Type::Int(Sign::Signed),
                                             Type::Int(Sign::Signed),
                                             Type::Int(Sign::Signed)],
                                        false);
        let llvm_memcpy_llvm_ty = LLVMFunctionType(LLVMVoidType(),
                                                   vec![LLVMPointerType(LLVMInt8Type(), 0),
                                                        LLVMPointerType(LLVMInt8Type(), 0),
                                                        LLVMInt32Type(),
                                                        LLVMInt32Type(),
                                                        LLVMInt1Type()]
                                                           .as_mut_slice()
                                                           .as_mut_ptr(),
                                                   5,
                                                   0);
        let llvm_memcpy = LLVMAddFunction(module,
                                          CString::new("llvm.memcpy.p0i8.p0i8.i32")
                                              .unwrap()
                                              .as_ptr(),
                                          llvm_memcpy_llvm_ty);
        global_varmap.insert("llvm.memcpy.p0i8.p0i8.i32".to_string(),
                             VarInfo::new(llvm_memcpy_ty, llvm_memcpy_llvm_ty, llvm_memcpy));

        Codegen {
            context: LLVMContextCreate(),
            module: module,
            builder: LLVMCreateBuilderInContext(LLVMContextCreate()),
            global_varmap: global_varmap,
            local_varmap: Vec::new(),
            llvm_struct_map: HashMap::new(),
            _data_layout: LLVMGetDataLayout(module),
            cur_func: None,
        }
    }

    pub unsafe fn run(&mut self, node: Vec<node::AST>) {
        for ast in node {
            match self.gen(&ast) {
                Ok(_) => {}
                Err(err) => {
                    match err {
                        Error::Msg(msg) => error::error_exit(ast.line, msg.as_str()),
                        Error::MsgWithLine(msg, line) => error::error_exit(line, msg.as_str()),
                    }
                }
            }
        }
        LLVMDumpModule(self.module);
    }

    pub unsafe fn write_llvmir_to_file(&mut self, filename: &str) {
        let mut errmsg = ptr::null_mut();
        if LLVMPrintModuleToFile(self.module,
                                 CString::new(filename).unwrap().as_ptr(),
                                 &mut errmsg) != 0 {
            let _err = CString::from_raw(errmsg)
                .to_owned()
                .into_string()
                .unwrap();
            std::process::exit(-1);
        }
    }

    pub unsafe fn gen(&mut self, ast: &node::AST) -> CodegenResult {
        let result = match ast.kind {
            node::ASTKind::FuncDef(ref functy, ref param_names, ref name, ref body) => {
                self.gen_func_def(functy, param_names, name, body)
            }
            node::ASTKind::VariableDecl(ref ty, ref name, ref sclass, ref init) => {
                self.gen_var_decl(ty, name, sclass, init)
            }
            node::ASTKind::Block(ref block) => self.gen_block(block),
            node::ASTKind::Compound(ref block) => self.gen_compound(block),
            node::ASTKind::If(ref cond, ref then_stmt, ref else_stmt) => {
                self.gen_if(&*cond, &*then_stmt, &*else_stmt)
            }
            node::ASTKind::For(ref init, ref cond, ref step, ref body) => {
                self.gen_for(&*init, &*cond, &*step, &*body)
            }
            node::ASTKind::While(ref cond, ref body) => self.gen_while(&*cond, &*body),
            node::ASTKind::UnaryOp(ref expr, ref op) => self.gen_unary_op(&*expr, op),
            node::ASTKind::BinaryOp(ref lhs, ref rhs, ref op) => {
                self.gen_binary_op(&**lhs, &**rhs, &*op)
            }
            node::ASTKind::TernaryOp(ref cond, ref lhs, ref rhs) => {
                self.gen_ternary_op(&*cond, &*lhs, &*rhs)
            }
            node::ASTKind::StructRef(ref expr, ref field_name) => {
                self.gen_struct_field(&*expr, field_name.to_string())
            }
            node::ASTKind::TypeCast(ref expr, ref ty) => self.gen_type_cast(expr, ty),
            node::ASTKind::Load(ref expr) => self.gen_load(expr),
            node::ASTKind::Variable(ref name) => self.gen_var(name),
            node::ASTKind::ConstArray(ref elems) => self.gen_const_array(elems),
            node::ASTKind::FuncCall(ref f, ref args) => self.gen_func_call(&*f, args),
            node::ASTKind::Return(ref ret) => {
                if ret.is_none() {
                    Ok((LLVMBuildRetVoid(self.builder), None))
                } else {
                    let (retval, _) = try!(self.gen(&*ret.clone().unwrap()));
                    self.gen_return(retval)
                }
            }
            node::ASTKind::Int(ref n) => self.make_int(*n as u64, false),
            node::ASTKind::Float(ref f) => self.make_double(*f),
            node::ASTKind::Char(ref c) => self.make_char(*c),
            node::ASTKind::String(ref s) => self.make_const_str(s),
            _ => {
                error::error_exit(0,
                                  format!("codegen: unknown ast (given {:?})", ast).as_str())
            }
        };
        result.or_else(|cr: Error| match cr {
                           Error::Msg(msg) => Err(Error::MsgWithLine(msg, ast.line)),
                           Error::MsgWithLine(msg, line) => Err(Error::MsgWithLine(msg, line)),
                       })
    }

    pub unsafe fn gen_func_def(&mut self,
                               functy: &Type,
                               param_names: &Vec<String>,
                               name: &String,
                               body: &Rc<node::AST>)
                               -> CodegenResult {
        let func_ty = self.type_to_llvmty(functy);
        let (func_retty, func_args_types, _func_is_vararg) = match functy {
            &Type::Func(ref retty, ref args_types, ref is_vararg) => (retty, args_types, is_vararg),
            _ => return Err(Error::Msg("gen_func_def: never reach!".to_string())),
        };
        let func = LLVMAddFunction(self.module,
                                   CString::new(name.as_str()).unwrap().as_ptr(),
                                   func_ty);
        self.global_varmap
            .insert(name.to_string(),
                    VarInfo::new(functy.clone(), func_ty, func));
        self.cur_func = Some(func);
        self.local_varmap.push(HashMap::new());

        let bb_entry = LLVMAppendBasicBlock(func, CString::new("entry").unwrap().as_ptr());
        LLVMPositionBuilderAtEnd(self.builder, bb_entry);

        for (i, (arg_ty, arg_name)) in
            func_args_types
                .iter()
                .zip(param_names.iter())
                .enumerate() {
            let arg_val = LLVMGetParam(func, i as u32);
            let var = try!(self.gen_local_var_decl(arg_ty, arg_name, &StorageClass::Auto, &None)).0;
            LLVMBuildStore(self.builder, arg_val, var);
        }


        try!(self.gen(&**body));

        let mut iter_bb = LLVMGetFirstBasicBlock(func);
        while iter_bb != ptr::null_mut() {
            if LLVMGetBasicBlockTerminator(iter_bb) == ptr::null_mut() {
                let terminator_builder = LLVMCreateBuilderInContext(self.context);
                LLVMPositionBuilderAtEnd(terminator_builder, iter_bb);
                LLVMBuildRet(terminator_builder,
                             LLVMConstNull(self.type_to_llvmty(func_retty)));
            }
            iter_bb = LLVMGetNextBasicBlock(iter_bb);
        }

        self.local_varmap.pop();

        self.cur_func = None;

        Ok((func, None))
    }

    unsafe fn gen_var_decl(&mut self,
                           ty: &Type,
                           name: &String,
                           sclass: &StorageClass,
                           init: &Option<Rc<node::AST>>)
                           -> CodegenResult {
        // is global
        if self.cur_func.is_none() {
            try!(self.gen_global_var_decl(ty, name, sclass, init));
        } else {
            try!(self.gen_local_var_decl(ty, name, sclass, init));
        }
        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_global_var_decl(&mut self,
                                  ty: &Type,
                                  name: &String,
                                  sclass: &StorageClass,
                                  init: &Option<Rc<node::AST>>)
                                  -> CodegenResult {
        let (gvar, llvm_gvar_ty) = if self.global_varmap.contains_key(name) {
            let ref v = self.global_varmap.get(name).unwrap();
            (v.llvm_val, v.llvm_ty)
        } else {
            match *ty {
                Type::Func(_, _, _) => {
                    let llvmty = self.type_to_llvmty(ty);
                    (LLVMAddFunction(self.module,
                                     CString::new(name.as_str()).unwrap().as_ptr(),
                                     llvmty),
                     llvmty)
                }
                _ => {
                    let llvmty = self.type_to_llvmty(ty);
                    (LLVMAddGlobal(self.module,
                                   self.type_to_llvmty(ty),
                                   CString::new(name.as_str()).unwrap().as_ptr()),
                     llvmty)
                }
            }
        };
        self.global_varmap
            .insert(name.to_string(),
                    VarInfo::new(ty.clone(), llvm_gvar_ty, gvar));

        if init.is_some() {
            self.const_init_global_var(ty, gvar, &*init.clone().unwrap())
        } else {
            match *ty {
                Type::Func(_, _, _) => return Ok((ptr::null_mut(), None)),
                _ => {}
            }

            LLVMSetLinkage(gvar,
                           match *sclass {
                               StorageClass::Typedef => panic!(),
                               StorageClass::Extern => llvm::LLVMLinkage::LLVMExternalLinkage,
                               StorageClass::Static => llvm::LLVMLinkage::LLVMCommonLinkage, // TODO: think handling of static
                               StorageClass::Register => llvm::LLVMLinkage::LLVMCommonLinkage,
                               StorageClass::Auto => llvm::LLVMLinkage::LLVMCommonLinkage,
                           });
            // TODO: implement correctly
            if *sclass == StorageClass::Auto {
                match *ty {
                    Type::Array(ref elem_ty, _) => {
                        LLVMSetInitializer(gvar,
                                           LLVMConstArray(self.type_to_llvmty(elem_ty),
                                                          ptr::null_mut(),
                                                          0))
                    }
                    Type::Struct(_, _) => {
                        LLVMSetInitializer(gvar, LLVMConstStruct(ptr::null_mut(), 0, 0))
                    }
                    _ => {}
                }
            }
            Ok((ptr::null_mut(), None))
        }
    }
    unsafe fn const_init_global_var(&mut self,
                                    ty: &Type,
                                    gvar: LLVMValueRef,
                                    init_ast: &node::AST)
                                    -> CodegenResult {
        match *ty {
            // TODO: support only if const array size is the same as var's array size
            Type::Array(ref _ary_ty, ref _len) => {
                LLVMSetInitializer(gvar, try!(self.gen(init_ast)).0);
                Ok((ptr::null_mut(), None))
            }
            _ => {
                LLVMSetInitializer(gvar, try!(self.gen(init_ast)).0);
                Ok((ptr::null_mut(), None))
            }
        }
    }
    unsafe fn gen_const_array(&mut self, elems_ast: &Vec<node::AST>) -> CodegenResult {
        let mut elems = Vec::new();
        let (elem_val, elem_ty) = try!(self.gen(&elems_ast[0]));
        elems.push(elem_val);
        for e in elems_ast[1..].iter() {
            elems.push(try!(self.gen(e)).0);
        }
        Ok((LLVMConstArray(LLVMTypeOf(elems[0]),
                           elems.as_mut_slice().as_mut_ptr(),
                           elems.len() as u32),
            Some(Type::Array(Rc::new(elem_ty.unwrap()), elems.len() as i32))))
    }
    unsafe fn gen_local_var_decl(&mut self,
                                 ty: &Type,
                                 name: &String,
                                 sclass: &StorageClass,
                                 init: &Option<Rc<node::AST>>)
                                 -> CodegenResult {
        // Allocate a varaible, always at the first of the entry block
        let func = self.cur_func.unwrap();
        let builder = LLVMCreateBuilderInContext(self.context);
        let entry_bb = LLVMGetEntryBasicBlock(func);
        let first_inst = LLVMGetFirstInstruction(entry_bb);
        if first_inst == ptr::null_mut() {
            LLVMPositionBuilderAtEnd(builder, entry_bb);
        } else {
            LLVMPositionBuilderBefore(builder, first_inst);
        }
        let llvm_var_ty = self.type_to_llvmty(ty);
        let var = LLVMBuildAlloca(builder,
                                  llvm_var_ty,
                                  CString::new(name.as_str()).unwrap().as_ptr());
        self.local_varmap
            .last_mut()
            .unwrap()
            .insert(name.as_str().to_string(),
                    VarInfo::new(ty.clone(), llvm_var_ty, var));

        assert!(*sclass != StorageClass::Static,
                "not supported 'static' for local var");

        if init.is_some() {
            try!(self.set_local_var_initializer(var, ty, &*init.clone().unwrap()));
        }
        Ok((var, Some(ty.clone())))
    }
    unsafe fn set_local_var_initializer(&mut self,
                                        var: LLVMValueRef,
                                        varty: &Type,
                                        init: &node::AST)
                                        -> CodegenResult {
        let init_val = try!(self.gen(init)).0;
        match *varty {
            Type::Array(_, _) => {
                let llvm_memcpy = self.global_varmap
                    .get("llvm.memcpy.p0i8.p0i8.i32")
                    .unwrap();
                let init_ary = LLVMAddGlobal(self.module,
                                             LLVMGetElementType(LLVMTypeOf(var)),
                                             CString::new("constary").unwrap().as_ptr());
                LLVMSetInitializer(init_ary, init_val);
                Ok((LLVMBuildCall(self.builder,
                                  llvm_memcpy.llvm_val,
                                  vec![self.typecast(var, LLVMPointerType(LLVMInt8Type(), 0)),
                                       self.typecast(init_ary,
                                                     LLVMPointerType(LLVMInt8Type(), 0)),
                                       LLVMConstInt(LLVMInt32Type(),
                                                    varty.calc_size() as u64,
                                                    0),
                                       LLVMConstInt(LLVMInt32Type(), 4, 0),
                                       LLVMConstInt(LLVMInt1Type(), 0, 0)]
                                          .as_mut_slice()
                                          .as_mut_ptr(),
                                  5,
                                  CString::new("").unwrap().as_ptr()),
                    None))
            }
            _ => Ok((LLVMBuildStore(self.builder, init_val, var), None)),
        }
    }

    unsafe fn gen_block(&mut self, block: &Vec<node::AST>) -> CodegenResult {
        self.local_varmap.push(HashMap::new());
        for stmt in block {
            try!(self.gen(stmt));
        }
        self.local_varmap.pop();
        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_compound(&mut self, block: &Vec<node::AST>) -> CodegenResult {
        for stmt in block {
            try!(self.gen(stmt));
        }
        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_if(&mut self,
                     cond: &node::AST,
                     then_stmt: &node::AST,
                     else_stmt: &node::AST)
                     -> CodegenResult {
        let cond_val = {
            let val = try!(self.gen(cond)).0;
            LLVMBuildICmp(self.builder,
                          llvm::LLVMIntPredicate::LLVMIntNE,
                          val,
                          LLVMConstNull(LLVMTypeOf(val)),
                          CString::new("cond").unwrap().as_ptr())
        };


        let func = self.cur_func.unwrap();

        let bb_then = LLVMAppendBasicBlock(func, CString::new("then").unwrap().as_ptr());
        let bb_else = LLVMAppendBasicBlock(func, CString::new("else").unwrap().as_ptr());
        let bb_merge = LLVMAppendBasicBlock(func, CString::new("merge").unwrap().as_ptr());

        LLVMBuildCondBr(self.builder, cond_val, bb_then, bb_else);

        LLVMPositionBuilderAtEnd(self.builder, bb_then);
        // then block
        try!(self.gen(then_stmt));
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_else);
        // else block
        try!(self.gen(else_stmt));
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_merge);

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_while(&mut self, cond: &node::AST, body: &node::AST) -> CodegenResult {
        let func = self.cur_func.unwrap();

        let bb_before_loop = LLVMAppendBasicBlock(func,
                                                  CString::new("before_loop").unwrap().as_ptr());
        let bb_loop = LLVMAppendBasicBlock(func, CString::new("loop").unwrap().as_ptr());
        let bb_after_loop = LLVMAppendBasicBlock(func,
                                                 CString::new("after_loop").unwrap().as_ptr());

        LLVMBuildBr(self.builder, bb_before_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_before_loop);
        // before_loop block
        let cond_val = {
            let val = try!(self.gen(cond)).0;
            LLVMBuildICmp(self.builder,
                          llvm::LLVMIntPredicate::LLVMIntNE,
                          val,
                          LLVMConstNull(LLVMTypeOf(val)),
                          CString::new("eql").unwrap().as_ptr())
        };
        LLVMBuildCondBr(self.builder, cond_val, bb_loop, bb_after_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_loop);
        // loop block
        try!(self.gen(body));
        LLVMBuildBr(self.builder, bb_before_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_after_loop);

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_for(&mut self,
                      init: &node::AST,
                      cond: &node::AST,
                      step: &node::AST,
                      body: &node::AST)
                      -> CodegenResult {
        let func = self.cur_func.unwrap();

        let bb_before_loop = LLVMAppendBasicBlock(func,
                                                  CString::new("before_loop").unwrap().as_ptr());
        let bb_loop = LLVMAppendBasicBlock(func, CString::new("loop").unwrap().as_ptr());
        let bb_after_loop = LLVMAppendBasicBlock(func,
                                                 CString::new("after_loop").unwrap().as_ptr());
        try!(self.gen(init));

        LLVMBuildBr(self.builder, bb_before_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_before_loop);
        // before_loop block
        let cond_val = {
            let val = {
                let v = try!(self.gen(cond)).0;
                if v == ptr::null_mut() {
                    try!(self.make_int(1, false)).0
                } else {
                    v
                }
            };
            LLVMBuildICmp(self.builder,
                          llvm::LLVMIntPredicate::LLVMIntNE,
                          val,
                          LLVMConstNull(LLVMTypeOf(val)),
                          CString::new("eql").unwrap().as_ptr())
        };
        LLVMBuildCondBr(self.builder, cond_val, bb_loop, bb_after_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_loop);
        // loop block
        try!(self.gen(body));
        try!(self.gen(step));
        LLVMBuildBr(self.builder, bb_before_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_after_loop);

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_unary_op(&mut self, expr: &node::AST, op: &node::CUnaryOps) -> CodegenResult {
        match *op {
            node::CUnaryOps::Deref => self.gen_load(expr),
            _ => Ok((ptr::null_mut(), None)),
        }
    }

    unsafe fn gen_binary_op(&mut self,
                            lhsast: &node::AST,
                            rhsast: &node::AST,
                            op: &node::CBinOps)
                            -> CodegenResult {
        match *op {
            // logical operators
            node::CBinOps::LAnd => {}
            node::CBinOps::LOr => {}
            // assignment
            node::CBinOps::Assign => {
                return self.gen_assign(lhsast, rhsast);
            }
            _ => {}
        }

        // normal binary operators
        let (lhs, lhsty_w) = try!(self.gen(lhsast));
        let (rhs, rhsty_w) = try!(self.gen(rhsast));
        let lhsty = if let Some(lhsty) = lhsty_w {
            lhsty
        } else {
            panic!("left hand side must return the type of expression");
        };
        let rhsty = if let Some(rhsty) = rhsty_w {
            rhsty
        } else {
            panic!("right hand side must return the type of expression");
        };

        let lhsty_sz = lhsty.calc_size();
        let rhsty_sz = rhsty.calc_size();

        match rhsty {
            Type::Ptr(elem_ty) |
            Type::Array(elem_ty, _) => {
                return self.gen_ptr_binary_op(rhs, lhs, Type::Ptr(elem_ty.clone()), op);
            }
            _ => {}
        }
        match lhsty {
            Type::Ptr(elem_ty) |
            Type::Array(elem_ty, _) => {
                return self.gen_ptr_binary_op(lhs, rhs, Type::Ptr(elem_ty.clone()), op);
            }
            Type::Char(_) | Type::Short(_) | Type::Int(_) | Type::Long(_) | Type::LLong(_) => {
                if lhsty_sz < rhsty_sz {
                    let castlhs = self.typecast(lhs, LLVMTypeOf(rhs));
                    return Ok((self.gen_int_binary_op(castlhs, rhs, op), Some(rhsty)));
                } else if lhsty_sz == rhsty_sz {
                    return Ok((self.gen_int_binary_op(lhs, rhs, op), Some(lhsty)));
                } else {
                    let castrhs = self.typecast(rhs, LLVMTypeOf(lhs));
                    return Ok((self.gen_int_binary_op(lhs, castrhs, op), Some(lhsty)));
                }
            }
            _ => {}
        }
        Err(Error::MsgWithLine("unsupported operation".to_string(), lhsast.line))
    }

    unsafe fn gen_assign(&mut self, lhsast: &node::AST, rhsast: &node::AST) -> CodegenResult {
        let (dst, ptr_dst_ty) = try!(self.gen(lhsast));
        // self.gen returns Ptr(real_type)
        let dst_ty = match ptr_dst_ty.unwrap().get_ptr_elem_ty() { 
            Some(ok) => ok,
            None => {
                return Err(Error::MsgWithLine("gen_assign: ptr_dst_ty must be a pointer to the value's type".to_string(),
                                              lhsast.line))
            }
        };
        let (src, _src_ty) = try!(self.gen(rhsast));
        let a = self.type_to_llvmty(&dst_ty);
        let casted_src = self.typecast(src, a);
        LLVMBuildStore(self.builder, casted_src, dst);
        Ok((LLVMBuildLoad(self.builder, dst, CString::new("load").unwrap().as_ptr()),
            Some((dst_ty).clone())))
    }

    unsafe fn gen_int_binary_op(&mut self,
                                lhs: LLVMValueRef,
                                rhs: LLVMValueRef,
                                op: &node::CBinOps)
                                -> LLVMValueRef {
        match *op {
            node::CBinOps::Add => {
                LLVMBuildAdd(self.builder,
                             lhs,
                             rhs,
                             CString::new("add").unwrap().as_ptr())
            }
            node::CBinOps::Sub => {
                LLVMBuildSub(self.builder,
                             lhs,
                             rhs,
                             CString::new("sub").unwrap().as_ptr())
            }
            node::CBinOps::Mul => {
                LLVMBuildMul(self.builder,
                             lhs,
                             rhs,
                             CString::new("mul").unwrap().as_ptr())
            }
            node::CBinOps::Div => {
                LLVMBuildSDiv(self.builder,
                              lhs,
                              rhs,
                              CString::new("div").unwrap().as_ptr())
            }
            node::CBinOps::Rem => {
                LLVMBuildSRem(self.builder,
                              lhs,
                              rhs,
                              CString::new("rem").unwrap().as_ptr())
            }
            node::CBinOps::Eq => {
                LLVMBuildICmp(self.builder,
                              llvm::LLVMIntPredicate::LLVMIntEQ,
                              lhs,
                              rhs,
                              CString::new("eql").unwrap().as_ptr())
            }
            node::CBinOps::Ne => {
                LLVMBuildICmp(self.builder,
                              llvm::LLVMIntPredicate::LLVMIntNE,
                              lhs,
                              rhs,
                              CString::new("ne").unwrap().as_ptr())
            }
            node::CBinOps::Lt => {
                LLVMBuildICmp(self.builder,
                              llvm::LLVMIntPredicate::LLVMIntSLT,
                              lhs,
                              rhs,
                              CString::new("lt").unwrap().as_ptr())
            }
            node::CBinOps::Gt => {
                LLVMBuildICmp(self.builder,
                              llvm::LLVMIntPredicate::LLVMIntSGT,
                              lhs,
                              rhs,
                              CString::new("gt").unwrap().as_ptr())
            }
            node::CBinOps::Le => {
                LLVMBuildICmp(self.builder,
                              llvm::LLVMIntPredicate::LLVMIntSLE,
                              lhs,
                              rhs,
                              CString::new("le").unwrap().as_ptr())
            }
            node::CBinOps::Ge => {
                LLVMBuildICmp(self.builder,
                              llvm::LLVMIntPredicate::LLVMIntSGE,
                              lhs,
                              rhs,
                              CString::new("ge").unwrap().as_ptr())
            }
            node::CBinOps::Shl => {
                LLVMBuildShl(self.builder,
                             lhs,
                             rhs,
                             CString::new("shl").unwrap().as_ptr())
            }
            node::CBinOps::Shr => {
                LLVMBuildAShr(self.builder,
                              lhs,
                              rhs,
                              CString::new("shr").unwrap().as_ptr())
            }
            node::CBinOps::And => {
                LLVMBuildAnd(self.builder,
                             lhs,
                             rhs,
                             CString::new("and").unwrap().as_ptr())
            }
            node::CBinOps::Or => {
                LLVMBuildOr(self.builder, lhs, rhs, CString::new("or").unwrap().as_ptr())
            }
            node::CBinOps::Xor => {
                LLVMBuildXor(self.builder,
                             lhs,
                             rhs,
                             CString::new("xor").unwrap().as_ptr())
            }
            _ => ptr::null_mut(),
        }
    }

    unsafe fn gen_ptr_binary_op(&mut self,
                                lhs: LLVMValueRef,
                                rhs: LLVMValueRef,
                                ty: Type,
                                op: &node::CBinOps)
                                -> CodegenResult {
        let mut numidx = vec![match *op {
                                  node::CBinOps::Add => rhs,
                                  node::CBinOps::Sub => {
                                      LLVMBuildSub(self.builder,
                                                   try!(self.make_int(0, false)).0,
                                                   rhs,
                                                   CString::new("sub").unwrap().as_ptr())
                                  }
                                  _ => rhs,
                              }];
        Ok((LLVMBuildGEP(self.builder,
                         lhs,
                         numidx.as_mut_slice().as_mut_ptr(),
                         1,
                         CString::new("add").unwrap().as_ptr()),
            Some(ty)))
    }

    unsafe fn gen_ternary_op(&mut self,
                             cond: &node::AST,
                             then_expr: &node::AST,
                             else_expr: &node::AST)
                             -> CodegenResult {
        let cond_val = {
            let val = try!(self.gen(cond)).0;
            LLVMBuildICmp(self.builder,
                          llvm::LLVMIntPredicate::LLVMIntNE,
                          val,
                          LLVMConstNull(LLVMTypeOf(val)),
                          CString::new("eql").unwrap().as_ptr())
        };


        let func = self.cur_func.unwrap();

        let bb_then = LLVMAppendBasicBlock(func, CString::new("then").unwrap().as_ptr());
        let bb_else = LLVMAppendBasicBlock(func, CString::new("else").unwrap().as_ptr());
        let bb_merge = LLVMAppendBasicBlock(func, CString::new("merge").unwrap().as_ptr());

        LLVMBuildCondBr(self.builder, cond_val, bb_then, bb_else);

        LLVMPositionBuilderAtEnd(self.builder, bb_then);
        // then block
        let (then_val, ty) = try!(self.gen(then_expr));
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_else);
        // else block
        let (else_val, _) = try!(self.gen(else_expr));
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_merge);

        let phi = LLVMBuildPhi(self.builder,
                               LLVMTypeOf(then_val),
                               CString::new("ternary_phi").unwrap().as_ptr());
        LLVMAddIncoming(phi,
                        vec![then_val].as_mut_slice().as_mut_ptr(),
                        vec![bb_then].as_mut_slice().as_mut_ptr(),
                        1);
        LLVMAddIncoming(phi,
                        vec![else_val].as_mut_slice().as_mut_ptr(),
                        vec![bb_else].as_mut_slice().as_mut_ptr(),
                        1);

        Ok((phi, ty))
    }

    unsafe fn gen_struct_field(&mut self, expr: &node::AST, field_name: String) -> CodegenResult {
        let (val, ptr_ty) = try!(self.get_struct_field_val(expr, field_name));
        Ok((val, ptr_ty))
    }
    unsafe fn get_struct_field_val(&mut self,
                                   expr: &node::AST,
                                   field_name: String)
                                   -> CodegenResult {
        let (strct, ptr_ty) = try!(self.gen(expr));
        let ty = ptr_ty.unwrap().get_ptr_elem_ty().or_else(|| 
            error::error_exit(expr.line,
                              "gen_assign: ptr_dst_ty must be a pointer to the value's type")).unwrap();
        let strct_name = self.get_struct_name(&ty);
        assert!(strct_name.is_some());

        // let &(ref fieldmap, ref fieldtypes, ref llvm_fieldtypes, llvm_struct_ty, is_struct) =
        let ref rectype = self.llvm_struct_map
            .get(strct_name.unwrap().as_str())
            .unwrap();
        let idx = *rectype.field_pos.get(field_name.as_str()).unwrap();
        if rectype.is_struct {
            Ok((LLVMBuildStructGEP(self.builder,
                                   strct,
                                   idx,
                                   CString::new("structref").unwrap().as_ptr()),
                Some(Type::Ptr(Rc::new(rectype.field_types[idx as usize].clone())))))
        } else {
            let llvm_idx_ty = rectype.field_llvm_types[idx as usize];
            Ok((self.typecast(strct, LLVMPointerType(llvm_idx_ty, 0)),
                Some(Type::Ptr(Rc::new(rectype.field_types[idx as usize].clone())))))
        }
    }

    unsafe fn gen_type_cast(&mut self, expr: &node::AST, ty: &Type) -> CodegenResult {
        let (val, _exprty) = try!(self.gen(expr));
        let llvm_ty = self.type_to_llvmty(ty);
        Ok((self.typecast(val, llvm_ty), Some(ty.clone())))
    }

    unsafe fn lookup_local_var(&mut self, name: &str) -> Option<&VarInfo> {
        if self.local_varmap.is_empty() {
            return None;
        }
        let mut n = (self.local_varmap.len() - 1) as i32;
        while n >= 0 {
            if self.local_varmap[n as usize].contains_key(name) {
                let varinfo = self.local_varmap[n as usize].get(name).unwrap();
                return Some(varinfo);
            }
            n -= 1;
        }
        None
    }

    unsafe fn gen_var(&mut self, name: &String) -> CodegenResult {
        let varinfo_w = self.lookup_var(name.as_str());
        match varinfo_w {
            Some(varinfo) => Ok((varinfo.llvm_val, Some(Type::Ptr(Rc::new(varinfo.ty))))),
            None => Err(Error::Msg(format!("gen_var: not found variable '{}'", name))),
        }
    }

    unsafe fn lookup_var(&mut self, name: &str) -> Option<VarInfo> {
        if let Some(varinfo) = self.lookup_local_var(name) {
            return Some(varinfo.clone());
        }
        if let Some(varinfo) = self.global_varmap.get(name) {
            return Some(varinfo.clone());
        }
        None
    }

    unsafe fn gen_load(&mut self, var: &node::AST) -> CodegenResult {
        let (val, ty) = try!(self.gen(var));

        println!("{:?}", ty);
        LLVMDumpValue(val);

        if let Some(Type::Ptr(ref elem_ty)) = ty {
            match **elem_ty {
                Type::Func(_, _, _) => return Ok((val, Some(Type::Ptr((*elem_ty).clone())))),
                Type::Array(ref ary_elemty, _) => {
                    return Ok((LLVMBuildGEP(self.builder,
                                            val,
                                            vec![try!(self.make_int(0, false)).0,
                                                 try!(self.make_int(0, false)).0]
                                                    .as_mut_slice()
                                                    .as_mut_ptr(),
                                            2,
                                            CString::new("gep").unwrap().as_ptr()),
                               Some(Type::Ptr(Rc::new((**ary_elemty).clone())))));
                }
                _ => {
                    return Ok((LLVMBuildLoad(self.builder,
                                             val,
                                             CString::new("var").unwrap().as_ptr()),
                               Some((**elem_ty).clone())));
                }
            }
        } else {
            panic!();
        }
    }

    unsafe fn gen_func_call(&mut self, ast: &node::AST, args: &Vec<node::AST>) -> CodegenResult {
        // there's a possibility that the types of args are not the same as the types of params.
        // so we call args before implicit type casting 'maybe incorrect args'.
        let mut maybe_incorrect_args_val = Vec::new();
        for arg in &*args {
            maybe_incorrect_args_val.push(try!(self.gen(arg)).0);
        }
        let args_len = args.len();

        let func = match ast.kind {
            node::ASTKind::Variable(ref name) => {
                let var = self.lookup_var(name);
                if let Some(varinfo) = var {
                    varinfo
                } else {
                    return Err(Error::MsgWithLine(format!("gen_func_call: not found function '{}'",
                                                          name),
                                                  ast.line));
                }
            }
            _ => {
                let (val, ty_w) = try!(self.gen(ast));
                VarInfo::new(ty_w.unwrap(), LLVMTypeOf(val), val)
            }
        };

        let (func_retty, func_params_types, func_is_vararg) = match func.ty {
            Type::Func(retty, params_types, is_vararg) => (retty, params_types, is_vararg),
            Type::Ptr(elemty) => {
                // func ptr
                if let Type::Func(retty, params_types, is_vararg) = (*elemty).clone() {
                    (retty, params_types, is_vararg)
                } else {
                    panic!();
                }
            }
            _ => panic!(),
        };

        let (llvm_func, llvm_functy) = match LLVMGetTypeKind(func.llvm_ty) {
            llvm::LLVMTypeKind::LLVMPointerTypeKind => {
                (LLVMBuildLoad(self.builder,
                               func.llvm_val,
                               CString::new("load").unwrap().as_ptr()),
                 LLVMGetElementType(func.llvm_ty))
            }
            _ => (func.llvm_val, func.llvm_ty),
        };

        let params_count = func_params_types.len();
        let mut args_val = Vec::new();
        let ptr_params_types = (&mut Vec::with_capacity(params_count)).as_mut_ptr();
        LLVMGetParamTypes(llvm_functy, ptr_params_types);
        let llvm_params_types = Vec::from_raw_parts(ptr_params_types, params_count, 0);

        // do implicit type casting
        if !func_is_vararg && params_count < args_len {
            error::error_exit(ast.line, "too many arguments");
        }
        if !func_is_vararg && params_count > args_len {
            error::error_exit(ast.line, "too little arguments");
        }
        for i in 0..args_len {
            args_val.push(if params_count <= i {
                              maybe_incorrect_args_val[i]
                          } else {
                              self.typecast(maybe_incorrect_args_val[i], llvm_params_types[i])
                          })
        }

        let args_val_ptr = args_val.as_mut_slice().as_mut_ptr();
        Ok((LLVMBuildCall(self.builder,
                          llvm_func,
                          args_val_ptr,
                          args_len as u32,
                          CString::new("funccall").unwrap().as_ptr()),
            Some((*func_retty).clone())))
    }

    unsafe fn gen_return(&mut self, retval: LLVMValueRef) -> CodegenResult {
        Ok((LLVMBuildRet(self.builder, retval), None))
    }

    pub unsafe fn make_int(&mut self, n: u64, is_unsigned: bool) -> CodegenResult {
        Ok((LLVMConstInt(LLVMInt32Type(), n, if is_unsigned { 1 } else { 0 }),
            Some(Type::Int(Sign::Signed))))
    }
    pub unsafe fn make_char(&mut self, n: i32) -> CodegenResult {
        Ok((LLVMConstInt(LLVMInt8Type(), n as u64, 0), Some(Type::Char(Sign::Signed))))
    }
    pub unsafe fn make_float(&mut self, f: f64) -> CodegenResult {
        Ok((LLVMConstReal(LLVMFloatType(), f), Some(Type::Float)))
    }
    pub unsafe fn make_double(&mut self, f: f64) -> CodegenResult {
        Ok((LLVMConstReal(LLVMDoubleType(), f), Some(Type::Double)))
    }
    pub unsafe fn make_const_str(&mut self, s: &String) -> CodegenResult {
        Ok((LLVMBuildGlobalStringPtr(self.builder,
                                     CString::new(s.as_str()).unwrap().as_ptr(),
                                     CString::new("str").unwrap().as_ptr()),
            Some(Type::Ptr(Rc::new(Type::Char(Sign::Signed))))))
    }


    // Functions Related to Types
    pub unsafe fn typecast(&self, val: LLVMValueRef, to: LLVMTypeRef) -> LLVMValueRef {
        let v_ty = LLVMTypeOf(val);
        let inst_name = CString::new("cast").unwrap().as_ptr();

        match LLVMGetTypeKind(v_ty) {
            llvm::LLVMTypeKind::LLVMIntegerTypeKind => {
                match LLVMGetTypeKind(to) {
                    llvm::LLVMTypeKind::LLVMIntegerTypeKind => {
                        let val_bw = LLVMGetIntTypeWidth(v_ty);
                        let to_bw = LLVMGetIntTypeWidth(to);
                        if val_bw < to_bw {
                            return LLVMBuildZExtOrBitCast(self.builder, val, to, inst_name);
                        }
                    }
                    llvm::LLVMTypeKind::LLVMDoubleTypeKind => {
                        return LLVMBuildSIToFP(self.builder, val, to, inst_name);
                    }
                    _ => {}
                }
            }
            llvm::LLVMTypeKind::LLVMDoubleTypeKind |
            llvm::LLVMTypeKind::LLVMFloatTypeKind => {
                return LLVMBuildFPToSI(self.builder, val, to, inst_name);
            }
            llvm::LLVMTypeKind::LLVMVoidTypeKind => return val,
            _ => {}
        }
        LLVMBuildTruncOrBitCast(self.builder, val, to, inst_name)
    }

    pub unsafe fn type_to_llvmty(&mut self, ty: &Type) -> LLVMTypeRef {
        match ty {
            &Type::Void => LLVMVoidType(),
            &Type::Char(_) => LLVMInt8Type(),
            &Type::Short(_) => LLVMInt16Type(),
            &Type::Int(_) => LLVMInt32Type(),
            &Type::Long(_) => LLVMInt32Type(),
            &Type::LLong(_) => LLVMInt64Type(),
            &Type::Float => LLVMFloatType(),
            &Type::Double => LLVMDoubleType(),
            &Type::Ptr(ref elemty) => {
                LLVMPointerType(|| -> LLVMTypeRef {
                                    let elemty = self.type_to_llvmty(&**elemty);
                                    match LLVMGetTypeKind(elemty) {
                                        llvm::LLVMTypeKind::LLVMVoidTypeKind => LLVMInt8Type(),
                                        _ => elemty,
                                    }
                                }(),
                                0)
            }
            &Type::Array(ref elemty, ref size) => {
                LLVMArrayType(self.type_to_llvmty(&**elemty), *size as u32)
            }
            &Type::Func(ref ret_type, ref param_types, ref is_vararg) => {
                LLVMFunctionType(self.type_to_llvmty(&**ret_type),
                                 || -> *mut LLVMTypeRef {
                                     let mut param_llvm_types: Vec<LLVMTypeRef> = Vec::new();
                                     for param_type in &*param_types {
                                         param_llvm_types.push(self.type_to_llvmty(&param_type));
                                     }
                                     param_llvm_types.as_mut_slice().as_mut_ptr()
                                 }(),
                                 (*param_types).len() as u32,
                                 if *is_vararg { 1 } else { 0 })
            }
            &Type::Struct(ref name, ref fields) => self.make_struct(name, fields),
            &Type::Union(ref name, ref fields, ref max_size_field_pos) => {
                self.make_union(name, fields, *max_size_field_pos)
            }
            &Type::Enum => LLVMInt32Type(),
        }
    }
    unsafe fn make_rectype_base(&mut self,
                                name: &String,
                                fields: &Vec<node::AST>,
                                fields_names_map: &mut HashMap<String, u32>,
                                fields_llvm_types: &mut Vec<LLVMTypeRef>,
                                fields_types: &mut Vec<Type>)
                                -> (bool, LLVMTypeRef) {
        // returns (does_the_rectype_already_exists?, LLVMStructType)
        let new_struct: LLVMTypeRef = {
            let strct = self.llvm_struct_map.get(name);
            if let Some(ref rectype) = strct {
                if !rectype.field_types.is_empty() {
                    // declared struct
                    return (true, rectype.llvm_rectype);
                } else {
                    rectype.llvm_rectype
                }
            } else {
                LLVMStructCreateNamed(self.context, CString::new(name.as_str()).unwrap().as_ptr())
            }
        };

        // 'fields' is Vec<AST>, field is AST
        for (i, field) in fields.iter().enumerate() {
            match field.kind {
                node::ASTKind::VariableDecl(ref ty, ref name, ref _sclass, ref _init) => {
                    fields_llvm_types.push(self.type_to_llvmty(ty));
                    fields_types.push(ty.clone());
                    fields_names_map.insert(name.to_string(), i as u32);
                }
                _ => error::error_exit(0, "impossible"),
            }
        }
        (false, new_struct)
    }
    unsafe fn make_struct(&mut self, name: &String, fields: &Vec<node::AST>) -> LLVMTypeRef {
        let mut fields_names_map: HashMap<String, u32> = HashMap::new();
        let mut fields_llvm_types: Vec<LLVMTypeRef> = Vec::new();
        let mut fields_types: Vec<Type> = Vec::new();
        let (exist, new_struct) =
            self.make_rectype_base(name,
                                   fields,
                                   &mut fields_names_map,
                                   &mut fields_llvm_types,
                                   &mut fields_types);
        if exist {
            return new_struct;
        }

        LLVMStructSetBody(new_struct,
                          fields_llvm_types.as_mut_slice().as_mut_ptr(),
                          fields_llvm_types.len() as u32,
                          0);
        self.llvm_struct_map
            .insert(name.to_string(),
                    RectypeInfo::new(fields_names_map,
                                     fields_types,
                                     fields_llvm_types,
                                     new_struct,
                                     true));
        new_struct
    }
    unsafe fn make_union(&mut self,
                         name: &String,
                         fields: &Vec<node::AST>,
                         max_size_field_pos: usize)
                         -> LLVMTypeRef {
        let mut fields_names_map: HashMap<String, u32> = HashMap::new();
        let mut fields_llvm_types: Vec<LLVMTypeRef> = Vec::new();
        let mut fields_types: Vec<Type> = Vec::new();
        let (exist, new_struct) =
            self.make_rectype_base(name,
                                   fields,
                                   &mut fields_names_map,
                                   &mut fields_llvm_types,
                                   &mut fields_types);
        if exist {
            return new_struct;
        }
        // size of an union is the same as the biggest type in the union
        LLVMStructSetBody(new_struct,
                          vec![fields_llvm_types[max_size_field_pos]]
                              .as_mut_slice()
                              .as_mut_ptr(),
                          1,
                          0);
        self.llvm_struct_map
            .insert(name.to_string(),
                    RectypeInfo::new(fields_names_map,
                                     fields_types,
                                     fields_llvm_types,
                                     new_struct,
                                     false));
        new_struct
    }
    unsafe fn get_struct_name(&mut self, ty: &Type) -> Option<String> {
        match ty {
            &Type::Struct(ref name, _) => Some(name.to_string()),
            &Type::Union(ref name, _, _) => Some(name.to_string()),
            _ => None,
        }
    }
}
