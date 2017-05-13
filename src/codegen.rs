extern crate llvm_sys as llvm;

// use std::mem;
use std::ffi::CString;
use std::ptr;
use std::rc::Rc;
use std;
use std::collections::HashMap;

use self::llvm::core::*;
use self::llvm::prelude::*;

// use parser;
// use lexer;
use node;
use types::{Type, Sign};
use error;

pub unsafe fn type_to_llvmty(ty: &Type) -> LLVMTypeRef {
    match ty {
        &Type::Void => LLVMVoidType(),
        &Type::Char(_) => LLVMInt8Type(),
        &Type::Short(_) => LLVMInt16Type(),
        &Type::Int(_) => LLVMInt32Type(),
        &Type::Long(_) => LLVMInt32Type(),
        &Type::LLong(_) => LLVMInt64Type(),
        &Type::Float => LLVMFloatType(),
        &Type::Double => LLVMDoubleType(),
        &Type::Ptr(ref elemty) => LLVMPointerType(type_to_llvmty(&**elemty), 0),
        &Type::Array(ref elemty, ref size) => {
            LLVMArrayType(type_to_llvmty(&**elemty), *size as u32)
        }
        &Type::Func(ref ret_type, ref param_types, ref is_vararg) => {
            LLVMFunctionType(type_to_llvmty(&**ret_type),
                             || -> *mut LLVMTypeRef {
                                 let mut param_llvm_types: Vec<LLVMTypeRef> = Vec::new();
                                 for param_type in &*param_types {
                                     param_llvm_types.push(type_to_llvmty(&param_type));
                                 }
                                 param_llvm_types.as_mut_slice().as_mut_ptr()
                             }(),
                             (*param_types).len() as u32,
                             if *is_vararg { 1 } else { 0 })
        }
    }
}

pub struct Codegen {
    context: LLVMContextRef,
    module: LLVMModuleRef,
    builder: LLVMBuilderRef,
    global_varmap: HashMap<String, (Type, LLVMValueRef)>,
    local_varmap: HashMap<String, (Type, LLVMValueRef)>,
    cur_func: Option<LLVMValueRef>,
}

impl Codegen {
    pub unsafe fn new(mod_name: &str) -> Codegen {
        let c_mod_name = CString::new(mod_name).unwrap();
        Codegen {
            context: LLVMContextCreate(),
            module: LLVMModuleCreateWithNameInContext(c_mod_name.as_ptr(), LLVMContextCreate()),
            builder: LLVMCreateBuilderInContext(LLVMContextCreate()),
            global_varmap: HashMap::new(),
            local_varmap: HashMap::new(),
            cur_func: None,
        }
    }

    pub unsafe fn run(&mut self, node: Vec<node::AST>) {
        for ast in node {
            self.gen(&ast);
        }

        // e.g. create func
        // let int_t = LLVMInt32TypeInContext(self.context);
        // let func_t = LLVMFunctionType(/* ret type = */
        //                               int_t,
        //                               /* param types = */
        //                               ptr::null_mut(),
        //                               /* param count = */
        //                               0,
        //                               /* is vararg? = */
        //                               0);
        // let main_func =
        //     LLVMAddFunction(self.module, CString::new("main").unwrap().as_ptr(), func_t);
        //
        // // create basic block 'entry'
        // let bb_entry = LLVMAppendBasicBlock(main_func, CString::new("entry").unwrap().as_ptr());
        //
        // LLVMPositionBuilderAtEnd(self.builder, bb_entry); // SetInsertPoint in C++
        //
        // LLVMBuildRet(self.builder, self.make_int(0, false));

        LLVMDumpModule(self.module);
    }

    pub unsafe fn write_llvmir_to_file(&mut self, filename: &str) {
        let mut errmsg = ptr::null_mut();
        if LLVMPrintModuleToFile(self.module,
                                 CString::new(filename).unwrap().as_ptr(),
                                 &mut errmsg) != 0 {
            let err = CString::from_raw(errmsg)
                .to_owned()
                .into_string()
                .unwrap();
            println!("write_to_file failed: {}", err);
            std::process::exit(-1);
        }
    }

    pub unsafe fn gen(&mut self, ast: &node::AST) -> (LLVMValueRef, Option<Type>) {
        match ast {
            &node::AST::FuncDef(ref functy, ref param_names, ref name, ref body) => {
                self.gen_func_def(functy, param_names, name, body)
            }
            &node::AST::VariableDecl(ref ty, ref name, ref init) => {
                self.gen_var_decl(ty, name, init)
            }
            &node::AST::Block(ref block) => self.gen_block(block),
            &node::AST::BinaryOp(ref lhs, ref rhs, ref op) => {
                self.gen_binary_op(&**lhs, &**rhs, &*op)
            }
            &node::AST::Variable(ref name) => self.gen_var(name),
            &node::AST::FuncCall(ref f, ref args) => self.gen_func_call(&*f, args),
            &node::AST::Return(ref ret) => {
                if ret.is_none() {
                    (LLVMBuildRetVoid(self.builder), None)
                } else {
                    let (retval, _) = self.gen(&*ret.clone().unwrap());
                    self.gen_return(retval)
                }
            }
            &node::AST::Int(ref n) => self.make_int(*n as u64, false),
            &node::AST::Float(ref f) => self.make_double(*f),
            &node::AST::Char(ref c) => self.make_char(*c),
            &node::AST::String(ref s) => self.make_const_str(s),
            _ => {
                error::error_exit(0,
                                  format!("codegen: unknown ast (given {:?})", ast).as_str())
            }
        }
    }

    pub unsafe fn gen_func_def(&mut self,
                               functy: &Type,
                               param_names: &Vec<String>,
                               name: &String,
                               body: &Rc<node::AST>)
                               -> (LLVMValueRef, Option<Type>) {
        let func_ty = type_to_llvmty(functy);
        let (func_retty, func_args_types, _func_is_vararg) = match functy {
            &Type::Func(ref retty, ref args_types, ref is_vararg) => (retty, args_types, is_vararg),
            _ => error::error_exit(0, "gen_func_def: never reach!"),
        };
        let func = LLVMAddFunction(self.module,
                                   CString::new(name.as_str()).unwrap().as_ptr(),
                                   func_ty);
        self.global_varmap
            .insert(name.to_string(), (functy.clone(), func));
        self.cur_func = Some(func);

        let bb_entry = LLVMAppendBasicBlock(func, CString::new("entry").unwrap().as_ptr());
        LLVMPositionBuilderAtEnd(self.builder, bb_entry);

        for (i, (arg_ty, arg_name)) in
            func_args_types
                .iter()
                .zip(param_names.iter())
                .enumerate() {
            let arg_val = LLVMGetParam(func, i as u32);
            let var = self.gen_local_var_decl(arg_ty, arg_name, &None);
            LLVMBuildStore(self.builder, arg_val, var);
        }


        self.gen(&**body);

        self.cur_func = None;

        (func, None)
    }

    unsafe fn gen_var_decl(&mut self,
                           ty: &Type,
                           name: &String,
                           init: &Option<Rc<node::AST>>)
                           -> (LLVMValueRef, Option<Type>) {
        // is global
        if self.cur_func.is_none() {
            self.gen_global_var_decl(ty, name, init);
        } else {
            self.gen_local_var_decl(ty, name, init);
        }
        (ptr::null_mut(), None)
    }

    unsafe fn gen_global_var_decl(&mut self,
                                  ty: &Type,
                                  name: &String,
                                  init: &Option<Rc<node::AST>>) {
        let gvar = match *ty {
            Type::Func(_, _, _) => {
                LLVMAddFunction(self.module,
                                CString::new(name.as_str()).unwrap().as_ptr(),
                                type_to_llvmty(ty))
            } 
            _ => {
                LLVMAddGlobal(self.module,
                              type_to_llvmty(ty),
                              CString::new(name.as_str()).unwrap().as_ptr())
            }
        };
        self.global_varmap
            .insert(name.to_string(), (ty.clone(), gvar));

        if init.is_some() {
            LLVMSetInitializer(gvar, self.gen(&*init.clone().unwrap()).0);
        }
    }
    unsafe fn gen_local_var_decl(&mut self,
                                 ty: &Type,
                                 name: &String,
                                 init: &Option<Rc<node::AST>>)
                                 -> LLVMValueRef {
        let m = LLVMBuildAlloca(self.builder,
                                type_to_llvmty(ty),
                                CString::new(name.as_str()).unwrap().as_ptr());
        self.local_varmap
            .insert(name.to_string(), (ty.clone(), m));

        if init.is_some() {
            self.set_local_var_initializer(m, ty, &*init.clone().unwrap());
        }
        m
    }
    unsafe fn set_local_var_initializer(&mut self,
                                        var: LLVMValueRef,
                                        varty: &Type,
                                        init: &node::AST) {
        let init_val = self.gen(init).0;
        LLVMBuildStore(self.builder, init_val, var);
    }

    unsafe fn gen_block(&mut self, block: &Vec<node::AST>) -> (LLVMValueRef, Option<Type>) {
        for stmt in block {
            self.gen(stmt);
        }
        (ptr::null_mut(), None)
    }

    unsafe fn gen_binary_op(&mut self,
                            lhsast: &node::AST,
                            rhsast: &node::AST,
                            op: &node::CBinOps)
                            -> (LLVMValueRef, Option<Type>) {
        // logical operators
        match *op {
            node::CBinOps::LAnd => {}
            node::CBinOps::LOr => {}
            _ => {}
        }

        // normal binary operators
        let (lhs, lhsty) = self.gen(lhsast);
        let (rhs, _rhsty) = self.gen(rhsast);
        match lhsty.unwrap() {
            Type::Int(_) => {
                return (self.gen_int_binary_op(lhs, rhs, op), Some(Type::Int(Sign::Signed)))
            }
            _ => {}
        }
        (ptr::null_mut(), Some(Type::Void))
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
                LLVMBuildAdd(self.builder,
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
            _ => ptr::null_mut(),
        }
    }

    unsafe fn gen_var(&mut self, name: &String) -> (LLVMValueRef, Option<Type>) {
        if self.local_varmap.contains_key(name.as_str()) {
            let &(ref ty, val) = self.local_varmap.get(name.as_str()).unwrap();
            return (LLVMBuildLoad(self.builder, val, CString::new("var").unwrap().as_ptr()),
                    Some(ty.clone()));
        }
        if self.global_varmap.contains_key(name.as_str()) {
            let &(ref ty, val) = self.global_varmap.get(name.as_str()).unwrap();
            return (LLVMBuildLoad(self.builder, val, CString::new("var").unwrap().as_ptr()),
                    Some(ty.clone()));
        }
        error::error_exit(0,
                          format!("gen_var: not found variable '{}'", name).as_str());
    }

    unsafe fn gen_func_call(&mut self,
                            fast: &node::AST,
                            args: &Vec<node::AST>)
                            -> (LLVMValueRef, Option<Type>) {
        // before implicit type casting. so 'maybe incorrect'
        let mut maybe_incorrect_args_val = Vec::new();
        for arg in &*args {
            maybe_incorrect_args_val.push(self.gen(arg).0);
        }
        let args_len = args.len();

        let maybe_func = match *fast {
            node::AST::Variable(ref name) => self.global_varmap.get(name),
            _ => None, // for func ptr
        };
        if maybe_func.is_none() {
            error::error_exit(0, "gen_func_call: not found func");
        }

        let functy = maybe_func.unwrap().0.clone();
        let (func_retty, func_args_types, _func_is_vararg) = match functy {
            Type::Func(retty, args_types, is_vararg) => (retty, args_types, is_vararg),
            _ => error::error_exit(0, "gen_func_call: never reach!"),
        };
        let func = maybe_func.unwrap().1;

        // do implicit type casting to args
        let mut args_val = Vec::new();
        for i in 0..args_len {
            args_val.push(if func_args_types.len() <= i {
                              maybe_incorrect_args_val[i]
                          } else {
                              self.typecast(maybe_incorrect_args_val[i],
                                            type_to_llvmty(&func_args_types[i]))
                          })
        }
        let args_val_ptr = args_val.as_mut_slice().as_mut_ptr();

        (LLVMBuildCall(self.builder,
                       func,
                       args_val_ptr,
                       args_len as u32,
                       CString::new("funccall").unwrap().as_ptr()),
         Some((*func_retty).clone()))
    }

    unsafe fn gen_return(&mut self, retval: LLVMValueRef) -> (LLVMValueRef, Option<Type>) {
        (LLVMBuildRet(self.builder, retval), None)
    }

    pub unsafe fn make_int(&mut self, n: u64, is_unsigned: bool) -> (LLVMValueRef, Option<Type>) {
        (LLVMConstInt(LLVMInt32Type(), n, if is_unsigned { 1 } else { 0 }),
         Some(Type::Int(Sign::Signed)))
    }
    pub unsafe fn make_char(&mut self, n: i32) -> (LLVMValueRef, Option<Type>) {
        (LLVMConstInt(LLVMInt8Type(), n as u64, 0), Some(Type::Char(Sign::Signed)))
    }
    pub unsafe fn make_float(&mut self, f: f64) -> (LLVMValueRef, Option<Type>) {
        (LLVMConstReal(LLVMFloatType(), f), Some(Type::Float))
    }
    pub unsafe fn make_double(&mut self, f: f64) -> (LLVMValueRef, Option<Type>) {
        (LLVMConstReal(LLVMDoubleType(), f), Some(Type::Double))
    }
    pub unsafe fn make_const_str(&mut self, s: &String) -> (LLVMValueRef, Option<Type>) {
        (LLVMBuildGlobalStringPtr(self.builder,
                                  CString::new(s.as_str()).unwrap().as_ptr(),
                                  CString::new("str").unwrap().as_ptr()),
         Some(Type::Ptr(Rc::new(Type::Char(Sign::Signed)))))
    }

    pub unsafe fn typecast(&self, v: LLVMValueRef, t: LLVMTypeRef) -> LLVMValueRef {
        let v_ty = LLVMTypeOf(v);
        let inst_name = CString::new("cast").unwrap().as_ptr();
        match LLVMGetTypeKind(v_ty) {
            llvm::LLVMTypeKind::LLVMIntegerTypeKind => {
                match LLVMGetTypeKind(t) {
                    llvm::LLVMTypeKind::LLVMIntegerTypeKind => {
                        let v_bw = LLVMGetIntTypeWidth(v_ty);
                        let t_bw = LLVMGetIntTypeWidth(t);
                        if v_bw < t_bw {
                            return LLVMBuildZExtOrBitCast(self.builder, v, t, inst_name);
                        }
                    }
                    llvm::LLVMTypeKind::LLVMDoubleTypeKind => {
                        return LLVMBuildSIToFP(self.builder, v, t, inst_name);
                    }
                    _ => {}
                }
            }
            llvm::LLVMTypeKind::LLVMDoubleTypeKind |
            llvm::LLVMTypeKind::LLVMFloatTypeKind => {
                return LLVMBuildFPToSI(self.builder, v, t, inst_name);
            }
            llvm::LLVMTypeKind::LLVMVoidTypeKind => return v,
            _ => {}
        }
        LLVMBuildTruncOrBitCast(self.builder, v, t, inst_name)
    }
}
