extern crate llvm_sys as llvm;

extern crate libc;

extern crate rand;
use self::rand::Rng;

use std::ffi::CString;
use std::ptr;
use std::boxed::Box;
use std::collections::{hash_map, HashMap, VecDeque};

use self::llvm::core::*;
use self::llvm::prelude::*;

use node;
use node::Bits;
use lexer::Pos;
use types::{Sign, StorageClass, Type};

macro_rules! matches {
    ($e:expr, $p:pat) => {
        match $e {
            $p => true,
            _ => false
        }
    }
}

pub fn retrieve_from_load<'a>(ast: &'a node::AST) -> &'a node::AST {
    match ast.kind {
        node::ASTKind::Load(ref var) | node::ASTKind::UnaryOp(ref var, node::CUnaryOps::Deref) => {
            var
        }
        _ => ast,
    }
}

// used by global_varmap and local_varmap
#[derive(Clone, Debug)]
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

#[derive(Clone)]
struct RectypeInfo {
    field_pos: HashMap<String, u32>,
    field_types: Vec<Type>,
    field_llvm_types: Vec<LLVMTypeRef>,
    llvm_rectype: LLVMTypeRef,
    is_struct: bool,
}
impl RectypeInfo {
    fn new(
        field_pos: HashMap<String, u32>,
        field_types: Vec<Type>,
        field_llvm_types: Vec<LLVMTypeRef>,
        llvm_rectype: LLVMTypeRef,
        is_struct: bool,
    ) -> RectypeInfo {
        RectypeInfo {
            field_pos: field_pos,
            field_types: field_types,
            field_llvm_types: field_llvm_types,
            llvm_rectype: llvm_rectype,
            is_struct: is_struct,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    MsgWithPos(String, Pos),
    Msg(String),
}

unsafe fn cur_bb_has_no_terminator(builder: LLVMBuilderRef) -> bool {
    LLVMIsATerminatorInst(LLVMGetLastInstruction(LLVMGetInsertBlock(builder))) == ptr::null_mut()
}

type CodegenR<T> = Result<T, Error>;

pub struct Codegen {
    context: LLVMContextRef,
    module: LLVMModuleRef,
    builder: LLVMBuilderRef,
    global_varmap: HashMap<String, VarInfo>,
    local_varmap: Vec<HashMap<String, VarInfo>>,
    label_map: HashMap<String, LLVMBasicBlockRef>,
    switch_list: VecDeque<(LLVMValueRef, LLVMBasicBlockRef, LLVMTypeRef)>,
    llvm_struct_map: HashMap<String, RectypeInfo>,
    break_labels: VecDeque<LLVMBasicBlockRef>,
    continue_labels: VecDeque<LLVMBasicBlockRef>,
    cur_func: Option<LLVMValueRef>,
}

impl Codegen {
    pub unsafe fn new(mod_name: &str) -> Codegen {
        llvm::execution_engine::LLVMLinkInInterpreter();
        llvm::target::LLVM_InitializeAllTargetMCs();
        llvm::target::LLVM_InitializeNativeTarget();
        llvm::target::LLVM_InitializeNativeAsmPrinter();
        llvm::target::LLVM_InitializeNativeAsmParser();

        let context = LLVMContextCreate();

        let c_mod_name = CString::new(mod_name).unwrap();
        let module = LLVMModuleCreateWithNameInContext(c_mod_name.as_ptr(), context);
        let mut global_varmap = HashMap::new();

        let mut ee = 0 as llvm::execution_engine::LLVMExecutionEngineRef;
        let mut error = 0 as *mut i8;
        if llvm::execution_engine::LLVMCreateExecutionEngineForModule(&mut ee, module, &mut error)
            != 0
        {
            println!("err");
        }

        let llvm_memcpy_ty = Type::Func(
            Box::new(Type::Void),
            vec![
                Type::Ptr(Box::new(Type::Char(Sign::Signed))),
                Type::Ptr(Box::new(Type::Char(Sign::Signed))),
                Type::Int(Sign::Signed),
                Type::Int(Sign::Signed),
                Type::Int(Sign::Signed),
            ],
            false,
        );
        let llvm_memcpy_llvm_ty = LLVMFunctionType(
            LLVMVoidType(),
            vec![
                LLVMPointerType(LLVMInt8Type(), 0),
                LLVMPointerType(LLVMInt8Type(), 0),
                LLVMInt32Type(),
                LLVMInt32Type(),
                LLVMInt1Type(),
            ].as_mut_slice()
                .as_mut_ptr(),
            5,
            0,
        );
        let llvm_memcpy = LLVMAddFunction(
            module,
            CString::new("llvm.memcpy.p0i8.p0i8.i32").unwrap().as_ptr(),
            llvm_memcpy_llvm_ty,
        );
        global_varmap.insert(
            "llvm.memcpy.p0i8.p0i8.i32".to_string(),
            VarInfo::new(llvm_memcpy_ty, llvm_memcpy_llvm_ty, llvm_memcpy),
        );

        let llvm_memset_ty = Type::Func(
            Box::new(Type::Void),
            vec![
                Type::Ptr(Box::new(Type::Char(Sign::Signed))),
                Type::Int(Sign::Signed),
                Type::Int(Sign::Signed),
                Type::Int(Sign::Signed),
                Type::Int(Sign::Signed),
            ],
            false,
        );
        let llvm_memset_llvm_ty = LLVMFunctionType(
            LLVMVoidType(),
            vec![
                LLVMPointerType(LLVMInt8Type(), 0),
                LLVMInt8Type(),
                LLVMInt32Type(),
                LLVMInt32Type(),
                LLVMInt1Type(),
            ].as_mut_slice()
                .as_mut_ptr(),
            5,
            0,
        );
        let llvm_memset = LLVMAddFunction(
            module,
            CString::new("llvm.memset.p0i8.i32").unwrap().as_ptr(),
            llvm_memset_llvm_ty,
        );
        global_varmap.insert(
            "llvm.memset.p0i8.i32".to_string(),
            VarInfo::new(llvm_memset_ty, llvm_memset_llvm_ty, llvm_memset),
        );

        Codegen {
            context: context,
            module: module,
            builder: LLVMCreateBuilderInContext(context),
            global_varmap: global_varmap,
            local_varmap: Vec::new(),
            label_map: HashMap::new(),
            switch_list: VecDeque::new(),
            llvm_struct_map: HashMap::new(),
            continue_labels: VecDeque::new(),
            break_labels: VecDeque::new(),
            cur_func: None,
        }
    }

    pub unsafe fn run(&mut self, node: &Vec<node::AST>) -> Result<(), Error> {
        for ast in node {
            match self.gen_toplevel(&ast) {
                Ok(_) => {}
                Err(err) => return Err(err),
            }
        }

        // LLVMDumpModule(self.module);
        Ok(())
    }

    pub unsafe fn write_llvm_bitcode_to_file(&mut self, filename: &str) {
        llvm::bit_writer::LLVMWriteBitcodeToFile(
            self.module,
            CString::new(filename).unwrap().as_ptr(),
        );
    }

    unsafe fn gen_toplevel(&mut self, ast: &node::AST) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let result = match ast.kind {
            node::ASTKind::FuncDef(ref functy, ref param_names, ref name, ref body) => {
                self.gen_func_def(functy, param_names, name, body)
            }
            node::ASTKind::VariableDecl(ref ty, ref name, ref sclass, ref init) => {
                self.gen_global_var_decl(ty, name, sclass, init)
            }
            _ => panic!(format!(
                "given AST which should not be contained in toplevel: {:?}",
                ast
            )),
        };

        result.or_else(|cr: Error| match cr {
            Error::Msg(msg) => Err(Error::MsgWithPos(msg, ast.pos.clone())),
            Error::MsgWithPos(msg, pos) => Err(Error::MsgWithPos(msg, pos)),
        })
    }

    unsafe fn gen(&mut self, ast: &node::AST) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let result = match ast.kind {
            node::ASTKind::VariableDecl(ref ty, ref name, ref sclass, ref init) => {
                self.gen_local_var_decl(ty, name, sclass, init)
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
            node::ASTKind::DoWhile(ref cond, ref body) => self.gen_do_while(&*cond, &*body),
            node::ASTKind::Switch(ref cond, ref body) => self.gen_switch(&*cond, &*body),
            node::ASTKind::Case(ref expr) => self.gen_case(&*expr),
            node::ASTKind::DefaultL => self.gen_default(),
            node::ASTKind::Goto(ref label_name) => self.gen_goto(label_name),
            node::ASTKind::Label(ref label_name) => self.gen_label(label_name),
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
            node::ASTKind::Variable(_, ref name) => self.gen_var(name),
            node::ASTKind::ConstArray(ref elems) => self.gen_const_array(elems),
            node::ASTKind::FuncCall(ref f, ref args) => self.gen_func_call(&*f, args),
            node::ASTKind::Continue => self.gen_continue(),
            node::ASTKind::Break => self.gen_break(),
            node::ASTKind::Return(Some(ref val)) => self.gen_return(val),
            node::ASTKind::Return(None) => Ok((LLVMBuildRetVoid(self.builder), None)),
            node::ASTKind::Int(ref n, ref bits) => self.make_int(*n as u64, &*bits, false),
            node::ASTKind::Float(ref f) => self.make_double(*f),
            node::ASTKind::Char(ref c) => self.make_char(*c),
            node::ASTKind::String(ref s) => self.make_const_str(s),
            _ => panic!(format!("codegen: unknown ast (given {:?})", ast)),
        };
        result.or_else(|cr: Error| match cr {
            Error::Msg(msg) => Err(Error::MsgWithPos(msg, ast.pos.clone())),
            Error::MsgWithPos(msg, pos) => Err(Error::MsgWithPos(msg, pos)),
        })
    }

    unsafe fn gen_init_global(
        &mut self,
        ast: &node::AST,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        match ast.kind {
            node::ASTKind::ConstArray(ref elems) => self.gen_const_array_for_global_init(elems, ty),
            node::ASTKind::ConstStruct(ref elems) => {
                self.gen_const_struct_for_global_init(elems, ty)
            }
            _ => self.gen(ast),
        }
    }

    unsafe fn gen_init_local(
        &mut self,
        var: LLVMValueRef,
        ast: &node::AST,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        match ast.kind {
            node::ASTKind::ConstArray(ref elems) => {
                self.gen_const_array_for_local_init(var, elems, ty)
            }
            node::ASTKind::ConstStruct(ref elems) => {
                self.gen_const_struct_for_local_init(var, elems, ty)
            }
            _ => {
                let val = try!(self.gen(ast)).0;
                Ok((
                    LLVMBuildStore(
                        self.builder,
                        self.typecast(val, (LLVMGetElementType(LLVMTypeOf(var)))),
                        var,
                    ),
                    None,
                ))
            }
        }
    }

    pub unsafe fn gen_func_def(
        &mut self,
        functy: &Type,
        param_names: &Vec<String>,
        name: &String,
        body: &Box<node::AST>,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func_ty = self.type_to_llvmty(functy);
        let (func_ret_ty, func_args_types, _func_is_vararg) = match functy {
            &Type::Func(ref ret_ty, ref args_types, ref is_vararg) => {
                (ret_ty, args_types, is_vararg)
            }
            _ => panic!("never reach"),
        };
        let func = match self.global_varmap.entry(name.to_string()) {
            hash_map::Entry::Occupied(o) => o.into_mut().llvm_val,
            hash_map::Entry::Vacant(v) => {
                let func = LLVMAddFunction(
                    self.module,
                    CString::new(name.as_str()).unwrap().as_ptr(),
                    func_ty,
                );
                v.insert(VarInfo::new(functy.clone(), func_ty, func));
                func
            }
        };

        self.cur_func = Some(func);
        self.local_varmap.push(HashMap::new());

        let bb_entry = LLVMAppendBasicBlock(func, CString::new("entry").unwrap().as_ptr());
        LLVMPositionBuilderAtEnd(self.builder, bb_entry);

        for (i, (arg_ty, arg_name)) in func_args_types.iter().zip(param_names.iter()).enumerate() {
            let arg_val = LLVMGetParam(func, i as u32);
            let var =
                try!(self.gen_local_var_decl(arg_ty, arg_name, &StorageClass::Auto, &None,)).0;
            LLVMBuildStore(self.builder, arg_val, var);
        }

        try!(self.gen(&**body));

        let mut iter_bb = LLVMGetFirstBasicBlock(func);
        while iter_bb != ptr::null_mut() {
            if LLVMIsATerminatorInst(LLVMGetLastInstruction(iter_bb)) == ptr::null_mut() {
                let terminator_builder = LLVMCreateBuilderInContext(self.context);
                LLVMPositionBuilderAtEnd(terminator_builder, iter_bb);
                match **func_ret_ty {
                    Type::Void => LLVMBuildRetVoid(terminator_builder),
                    _ => LLVMBuildRet(
                        terminator_builder,
                        LLVMConstNull(self.type_to_llvmty(func_ret_ty)),
                    ),
                };
            }
            iter_bb = LLVMGetNextBasicBlock(iter_bb);
        }

        self.local_varmap.pop();

        self.cur_func = None;

        Ok((func, None))
    }

    unsafe fn gen_global_var_decl(
        &mut self,
        ty: &Type,
        name: &String,
        sclass: &StorageClass,
        init: &Option<Box<node::AST>>,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let (gvar, llvm_gvar_ty) = if self.global_varmap.contains_key(name) {
            let ref v = self.global_varmap.get(name).unwrap();
            (v.llvm_val, v.llvm_ty)
        } else {
            match *ty {
                Type::Func(_, _, _) => {
                    let llvmty = self.type_to_llvmty(ty);
                    (
                        LLVMAddFunction(
                            self.module,
                            CString::new(name.as_str()).unwrap().as_ptr(),
                            llvmty,
                        ),
                        llvmty,
                    )
                }
                _ => {
                    let llvmty = self.type_to_llvmty(ty);
                    (
                        LLVMAddGlobal(
                            self.module,
                            self.type_to_llvmty(ty),
                            CString::new(name.as_str()).unwrap().as_ptr(),
                        ),
                        llvmty,
                    )
                }
            }
        };
        self.global_varmap.insert(
            name.to_string(),
            VarInfo::new(ty.clone(), llvm_gvar_ty, gvar),
        );

        if init.is_some() {
            self.const_init_global_var(ty, gvar, &*init.clone().unwrap())
        } else {
            // default initialization

            match *ty {
                // function is not initialized
                Type::Func(_, _, _) => return Ok((gvar, Some(ty.clone()))),
                _ => {}
            }

            LLVMSetLinkage(
                gvar,
                match *sclass {
                    StorageClass::Typedef => panic!(),
                    StorageClass::Extern => llvm::LLVMLinkage::LLVMExternalLinkage,
                    StorageClass::Static => llvm::LLVMLinkage::LLVMInternalLinkage, // TODO: think handling of static
                    StorageClass::Register => llvm::LLVMLinkage::LLVMCommonLinkage,
                    StorageClass::Auto => llvm::LLVMLinkage::LLVMCommonLinkage,
                },
            );
            // TODO: implement correctly
            if *sclass == StorageClass::Auto || *sclass == StorageClass::Static {
                LLVMSetInitializer(gvar, LLVMConstNull(self.type_to_llvmty(ty)));
            }
            Ok((gvar, Some(ty.clone())))
        }
    }
    unsafe fn const_init_global_var(
        &mut self,
        ty: &Type,
        gvar: LLVMValueRef,
        init_ast: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let init_val = try!(self.gen_init_global(init_ast, ty)).0;

        match *ty {
            // TODO: support only if const array size is the same as var's array size
            Type::Struct(_, _) | Type::Union(_, _, _) | Type::Array(_, _) => {
                LLVMSetInitializer(gvar, init_val);
                Ok((gvar, Some(ty.clone())))
            }
            _ => {
                let cast_ty = LLVMGetElementType(LLVMTypeOf(gvar));
                LLVMSetInitializer(gvar, self.typecast(init_val, cast_ty));
                Ok((gvar, Some(ty.clone())))
            }
        }
    }
    unsafe fn gen_const_array(
        &mut self,
        elems_ast: &Vec<node::AST>,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let mut elems = Vec::new();
        let (elem_val, elem_ty) = try!(self.gen(&elems_ast[0]));
        elems.push(elem_val);
        let llvm_elem_ty = LLVMTypeOf(elems[0]);
        for e in elems_ast[1..].iter() {
            let elem = try!(self.gen(e)).0;
            elems.push(self.typecast(elem, llvm_elem_ty));
        }
        Ok((
            LLVMConstArray(
                llvm_elem_ty,
                elems.as_mut_slice().as_mut_ptr(),
                elems.len() as u32,
            ),
            Some(Type::Array(Box::new(elem_ty.unwrap()), elems.len() as i32)),
        ))
    }
    unsafe fn gen_const_array_for_global_init(
        &mut self,
        elems_ast: &Vec<node::AST>,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let (elem_ty, len) = if let &Type::Array(ref elem_ty, len) = ty {
            (&**elem_ty, len)
        } else {
            panic!("never reach");
        };

        let llvm_elem_ty = self.type_to_llvmty(elem_ty);
        let mut elems = Vec::new();
        for e in elems_ast {
            let elem = try!(self.gen_init_global(e, elem_ty)).0;
            elems.push(self.typecast(elem, llvm_elem_ty));
        }
        for _ in 0..(len - elems_ast.len() as i32) {
            elems.push(LLVMConstNull(llvm_elem_ty));
        }
        Ok((
            LLVMConstArray(llvm_elem_ty, elems.as_mut_slice().as_mut_ptr(), len as u32),
            Some(ty.clone()),
        ))
    }
    unsafe fn gen_const_struct_for_global_init(
        &mut self,
        elems_ast: &Vec<node::AST>,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let struct_name = ty.get_name().unwrap();
        let rectype = (*self.llvm_struct_map.get(struct_name.as_str()).unwrap()).clone();

        let mut elems = Vec::new();
        for ((elem_ast, field_ty), field_llvm_ty) in elems_ast
            .iter()
            .zip(rectype.field_types.iter())
            .zip(rectype.field_llvm_types.iter())
        {
            let elem_val = try!(self.gen_init_global(elem_ast, field_ty)).0;
            elems.push(self.typecast(elem_val, *field_llvm_ty));
        }
        Ok((
            LLVMConstNamedStruct(
                rectype.llvm_rectype,
                elems.as_mut_slice().as_mut_ptr(),
                elems_ast.len() as u32,
            ),
            Some(ty.clone()),
        ))
    }
    unsafe fn fill_with_0(
        &mut self,
        var: LLVMValueRef,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let llvm_memset = self.global_varmap
            .get("llvm.memset.p0i8.i32")
            .unwrap()
            .clone();
        LLVMBuildCall(
            self.builder,
            llvm_memset.llvm_val,
            vec![
                self.typecast(var, LLVMPointerType(LLVMInt8Type(), 0)),
                try!(self.make_int(0, &Bits::Bits8, false)).0,
                LLVMConstInt(LLVMInt32Type(), ty.calc_size() as u64, 0),
                LLVMConstInt(LLVMInt32Type(), 4, 0),
                LLVMConstInt(LLVMInt1Type(), 0, 0),
            ].as_mut_slice()
                .as_mut_ptr(),
            5,
            CString::new("").unwrap().as_ptr(),
        );
        Ok((ptr::null_mut(), None))
    }
    unsafe fn gen_const_array_for_local_init(
        &mut self,
        var: LLVMValueRef,
        elems_ast: &Vec<node::AST>,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let elem_ty = ty.get_elem_ty().unwrap();

        try!(self.fill_with_0(var, ty));

        for (i, e) in elems_ast.iter().enumerate() {
            // TODO: makes very very no sense...
            let load = LLVMBuildGEP(
                self.builder,
                var,
                vec![
                    try!(self.make_int(0, &Bits::Bits32, false)).0,
                    try!(self.make_int(0, &Bits::Bits32, false)).0,
                ].as_mut_slice()
                    .as_mut_ptr(),
                2,
                CString::new("gep").unwrap().as_ptr(),
            );
            let idx = LLVMBuildGEP(
                self.builder,
                load,
                vec![try!(self.make_int(i as u64, &Bits::Bits32, false)).0]
                    .as_mut_slice()
                    .as_mut_ptr(),
                1,
                CString::new("gep").unwrap().as_ptr(),
            );
            try!(self.gen_init_local(idx, e, elem_ty));
        }
        Ok((ptr::null_mut(), None))
    }
    unsafe fn gen_const_struct_for_local_init(
        &mut self,
        var: LLVMValueRef,
        elems_ast: &Vec<node::AST>,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let struct_name = ty.get_name().unwrap();
        let rectype = (*self.llvm_struct_map.get(struct_name.as_str()).unwrap()).clone();

        try!(self.fill_with_0(var, ty));

        for (i, (elem_ast, field_ty)) in
            elems_ast.iter().zip(rectype.field_types.iter()).enumerate()
        {
            let idx = LLVMBuildStructGEP(
                self.builder,
                var,
                i as u32,
                CString::new("structref").unwrap().as_ptr(),
            );
            try!(self.gen_init_local(idx, elem_ast, field_ty));
        }
        Ok((ptr::null_mut(), None))
    }
    unsafe fn gen_local_var_decl(
        &mut self,
        ty: &Type,
        name: &String,
        sclass: &StorageClass,
        init: &Option<Box<node::AST>>,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let llvm_var_ty = self.type_to_llvmty(ty);

        // TODO: refine code
        if *sclass == StorageClass::Static {
            let randid: String = rand::thread_rng().gen_ascii_chars().take(8).collect();
            let (static_var, static_var_ty) = try!(self.gen_global_var_decl(
                ty,
                &format!("x{}.{}", randid, name),
                &StorageClass::Auto,
                init,
            ));
            self.local_varmap.last_mut().unwrap().insert(
                name.as_str().to_string(),
                VarInfo::new(ty.clone(), llvm_var_ty, static_var),
            );
            return Ok((static_var, static_var_ty));
        }

        // Allocate a varaible, always at the first of the entry block
        let func = self.cur_func.unwrap();
        let builder = LLVMCreateBuilderInContext(self.context);
        let entry_bb = LLVMGetEntryBasicBlock(func);
        let first_inst = LLVMGetFirstInstruction(entry_bb);
        // local var is always declared at the first of entry block
        if first_inst == ptr::null_mut() {
            LLVMPositionBuilderAtEnd(builder, entry_bb);
        } else {
            LLVMPositionBuilderBefore(builder, first_inst);
        }
        let var = LLVMBuildAlloca(
            builder,
            llvm_var_ty,
            CString::new(name.as_str()).unwrap().as_ptr(),
        );
        self.local_varmap.last_mut().unwrap().insert(
            name.as_str().to_string(),
            VarInfo::new(ty.clone(), llvm_var_ty, var),
        );

        if init.is_some() {
            try!(self.set_local_var_initializer(var, ty, &*init.clone().unwrap(),));
        }
        Ok((var, Some(ty.clone())))
    }
    unsafe fn set_local_var_initializer(
        &mut self,
        var: LLVMValueRef,
        varty: &Type,
        init: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        self.gen_init_local(var, init, varty)
    }

    unsafe fn gen_block(
        &mut self,
        block: &Vec<node::AST>,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        self.local_varmap.push(HashMap::new());
        for stmt in block {
            try!(self.gen(stmt));
        }
        self.local_varmap.pop();
        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_compound(
        &mut self,
        block: &Vec<node::AST>,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        for stmt in block {
            try!(self.gen(stmt));
        }
        Ok((ptr::null_mut(), None))
    }

    unsafe fn val_to_bool(&mut self, val: LLVMValueRef) -> LLVMValueRef {
        match LLVMGetTypeKind(LLVMTypeOf(val)) {
            llvm::LLVMTypeKind::LLVMDoubleTypeKind | llvm::LLVMTypeKind::LLVMFloatTypeKind => {
                LLVMBuildFCmp(
                    self.builder,
                    llvm::LLVMRealPredicate::LLVMRealONE,
                    val,
                    LLVMConstNull(LLVMTypeOf(val)),
                    CString::new("to_bool").unwrap().as_ptr(),
                )
            }
            _ => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntNE,
                val,
                LLVMConstNull(LLVMTypeOf(val)),
                CString::new("to_bool").unwrap().as_ptr(),
            ),
        }
    }
    unsafe fn val_to_bool_not(&mut self, val: LLVMValueRef) -> LLVMValueRef {
        match LLVMGetTypeKind(LLVMTypeOf(val)) {
            llvm::LLVMTypeKind::LLVMDoubleTypeKind | llvm::LLVMTypeKind::LLVMFloatTypeKind => {
                LLVMBuildFCmp(
                    self.builder,
                    llvm::LLVMRealPredicate::LLVMRealOEQ,
                    val,
                    LLVMConstNull(LLVMTypeOf(val)),
                    CString::new("to_bool").unwrap().as_ptr(),
                )
            }
            _ => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntEQ,
                val,
                LLVMConstNull(LLVMTypeOf(val)),
                CString::new("to_bool").unwrap().as_ptr(),
            ),
        }
    }

    unsafe fn gen_if(
        &mut self,
        cond: &node::AST,
        then_stmt: &node::AST,
        else_stmt: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let cond_val_tmp = try!(self.gen(cond)).0;
        let cond_val = self.val_to_bool(cond_val_tmp);

        let func = self.cur_func.unwrap();

        let bb_then = LLVMAppendBasicBlock(func, CString::new("then").unwrap().as_ptr());
        let bb_else = LLVMAppendBasicBlock(func, CString::new("else").unwrap().as_ptr());
        let bb_merge = LLVMAppendBasicBlock(func, CString::new("merge").unwrap().as_ptr());

        LLVMBuildCondBr(self.builder, cond_val, bb_then, bb_else);

        LLVMPositionBuilderAtEnd(self.builder, bb_then);
        // then block
        try!(self.gen(then_stmt));
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, bb_merge);
        }

        LLVMPositionBuilderAtEnd(self.builder, bb_else);
        // else block
        try!(self.gen(else_stmt));
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, bb_merge);
        }

        LLVMPositionBuilderAtEnd(self.builder, bb_merge);

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_while(
        &mut self,
        cond: &node::AST,
        body: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func = self.cur_func.unwrap();

        let bb_before_loop =
            LLVMAppendBasicBlock(func, CString::new("before_loop").unwrap().as_ptr());
        let bb_loop = LLVMAppendBasicBlock(func, CString::new("loop").unwrap().as_ptr());
        let bb_after_loop =
            LLVMAppendBasicBlock(func, CString::new("after_loop").unwrap().as_ptr());
        self.continue_labels.push_back(bb_loop);
        self.break_labels.push_back(bb_after_loop);

        LLVMBuildBr(self.builder, bb_before_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_before_loop);
        // before_loop block
        let cond_val_tmp = try!(self.gen(cond)).0;
        let cond_val = self.val_to_bool(cond_val_tmp);
        LLVMBuildCondBr(self.builder, cond_val, bb_loop, bb_after_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_loop);
        try!(self.gen(body));

        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, bb_before_loop);
        }

        LLVMPositionBuilderAtEnd(self.builder, bb_after_loop);
        self.continue_labels.pop_back();
        self.break_labels.pop_back();

        Ok((ptr::null_mut(), None))
    }
    unsafe fn gen_do_while(
        &mut self,
        cond: &node::AST,
        body: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func = self.cur_func.unwrap();

        let bb_before_loop =
            LLVMAppendBasicBlock(func, CString::new("before_loop").unwrap().as_ptr());
        let bb_loop = LLVMAppendBasicBlock(func, CString::new("loop").unwrap().as_ptr());
        let bb_after_loop =
            LLVMAppendBasicBlock(func, CString::new("after_loop").unwrap().as_ptr());
        self.continue_labels.push_back(bb_loop);
        self.break_labels.push_back(bb_after_loop);

        LLVMBuildBr(self.builder, bb_before_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_before_loop);
        LLVMBuildBr(self.builder, bb_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_loop);
        try!(self.gen(body));

        if cur_bb_has_no_terminator(self.builder) {
            let cond_val_tmp = try!(self.gen(cond)).0;
            let cond_val = self.val_to_bool(cond_val_tmp);
            LLVMBuildCondBr(self.builder, cond_val, bb_before_loop, bb_after_loop);
        }

        LLVMPositionBuilderAtEnd(self.builder, bb_after_loop);
        self.continue_labels.pop_back();
        self.break_labels.pop_back();

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_switch(
        &mut self,
        cond: &node::AST,
        body: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func = self.cur_func.unwrap();
        let cond_val = try!(self.gen(cond)).0;
        let bb_after_switch =
            LLVMAppendBasicBlock(func, CString::new("after_switch").unwrap().as_ptr());
        let default = LLVMAppendBasicBlock(func, CString::new("default").unwrap().as_ptr());
        let switch = LLVMBuildSwitch(self.builder, cond_val, default, 10);
        self.break_labels.push_back(bb_after_switch);
        self.switch_list
            .push_back((switch, default, LLVMTypeOf(cond_val)));

        try!(self.gen(body));

        // if the last (case|default) doesn't have 'break'
        // switch(X) {
        //  (case A|default): <--- this
        //   puts("...");
        // }
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, bb_after_switch);
        }

        // TODO: if d is null, that means default block exists. BUT not good implementation
        //       Is there a good function like 'GetInsertPoint'?
        let d = self.switch_list.pop_back().unwrap().1;
        if d != ptr::null_mut() {
            LLVMPositionBuilderAtEnd(self.builder, default);
            LLVMBuildBr(self.builder, bb_after_switch);
        }

        LLVMPositionBuilderAtEnd(self.builder, bb_after_switch);

        self.break_labels.pop_back();
        Ok((ptr::null_mut(), None))
    }
    unsafe fn gen_case(&mut self, expr: &node::AST) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func = self.cur_func.unwrap();
        let expr_val = try!(self.gen(expr)).0;
        let (switch, _, ty) = *self.switch_list.back().unwrap();
        let label = LLVMAppendBasicBlock(func, CString::new("label").unwrap().as_ptr());

        // if the above case doesn't have 'break'
        // switch(X) {
        //  case A:
        //  case B: <--- this
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, label);
        }

        LLVMPositionBuilderAtEnd(self.builder, label);
        LLVMAddCase(switch, self.typecast(expr_val, ty), label);
        Ok((ptr::null_mut(), None))
    }
    unsafe fn gen_default(&mut self) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let default = self.switch_list.back_mut().unwrap();

        // if the above case doesn't have 'break'
        // switch(X) {
        //  case A:
        //  default: <--- this
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, (*default).1);
        }

        LLVMPositionBuilderAtEnd(self.builder, (*default).1);
        // TODO: danger... refer to gen_switch...
        (*default).1 = ptr::null_mut();

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_for(
        &mut self,
        init: &node::AST,
        cond: &node::AST,
        step: &node::AST,
        body: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func = self.cur_func.unwrap();

        let bb_before_loop =
            LLVMAppendBasicBlock(func, CString::new("before_loop").unwrap().as_ptr());
        let bb_loop = LLVMAppendBasicBlock(func, CString::new("loop").unwrap().as_ptr());
        let bb_step = LLVMAppendBasicBlock(func, CString::new("step").unwrap().as_ptr());
        let bb_after_loop =
            LLVMAppendBasicBlock(func, CString::new("after_loop").unwrap().as_ptr());
        self.continue_labels.push_back(bb_step);
        self.break_labels.push_back(bb_after_loop);
        try!(self.gen(init));

        LLVMBuildBr(self.builder, bb_before_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_before_loop);
        // before_loop block
        let cond_val = {
            let val = {
                let v = try!(self.gen(cond)).0;
                if v == ptr::null_mut() {
                    try!(self.make_int(1, &Bits::Bits32, false)).0
                } else {
                    v
                }
            };
            self.val_to_bool(val)
        };
        LLVMBuildCondBr(self.builder, cond_val, bb_loop, bb_after_loop);

        LLVMPositionBuilderAtEnd(self.builder, bb_loop);

        try!(self.gen(body));
        LLVMBuildBr(self.builder, bb_step);

        LLVMPositionBuilderAtEnd(self.builder, bb_step);
        try!(self.gen(step));
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, bb_before_loop);
        }

        LLVMPositionBuilderAtEnd(self.builder, bb_after_loop);
        self.continue_labels.pop_back();
        self.break_labels.pop_back();

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_goto(&mut self, label_name: &String) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func = self.cur_func.unwrap();
        match self.label_map.entry(label_name.to_string()) {
            hash_map::Entry::Occupied(o) => {
                let label = o.into_mut();
                LLVMBuildBr(self.builder, *label);
            }
            hash_map::Entry::Vacant(v) => {
                let label = LLVMAppendBasicBlock(func, CString::new("label").unwrap().as_ptr());
                LLVMBuildBr(self.builder, label);
                v.insert(label);
            }
        };
        let tmp_label = LLVMAppendBasicBlock(func, CString::new("tmp_label").unwrap().as_ptr());
        LLVMPositionBuilderAtEnd(self.builder, tmp_label);

        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_label(&mut self, label_name: &String) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let func = self.cur_func.unwrap();
        match self.label_map.entry(label_name.to_string()) {
            hash_map::Entry::Occupied(o) => {
                let label = o.into_mut();
                LLVMPositionBuilderAtEnd(self.builder, *label);
            }
            hash_map::Entry::Vacant(v) => {
                let label = LLVMAppendBasicBlock(func, CString::new("label").unwrap().as_ptr());
                v.insert(label);
                LLVMBuildBr(self.builder, label);
                LLVMPositionBuilderAtEnd(self.builder, label);
            }
        };
        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_unary_op(
        &mut self,
        expr: &node::AST,
        op: &node::CUnaryOps,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        match *op {
            node::CUnaryOps::LNot => {
                let (val, ty) = try!(self.gen(expr));
                Ok((self.val_to_bool_not(val), ty))
            }
            node::CUnaryOps::BNot => {
                let (val, ty) = try!(self.gen(expr));
                // bitwise-not learned from clang'
                let minus_one = LLVMBuildSub(
                    self.builder,
                    try!(self.make_int(0, &Bits::Bits32, false)).0,
                    try!(self.make_int(1, &Bits::Bits32, false)).0,
                    CString::new("sub").unwrap().as_ptr(),
                );
                Ok((
                    LLVMBuildXor(
                        self.builder,
                        val,
                        minus_one,
                        CString::new("bitwisenot").unwrap().as_ptr(),
                    ),
                    ty,
                ))
            }
            node::CUnaryOps::Deref => self.gen_load(expr),
            node::CUnaryOps::Addr => self.gen(retrieve_from_load(expr)),
            node::CUnaryOps::Minus => {
                let (val, ty) = try!(self.gen(expr));
                Ok((
                    LLVMBuildNeg(self.builder, val, CString::new("minus").unwrap().as_ptr()),
                    ty,
                ))
            }
            node::CUnaryOps::Inc => {
                let before_inc = try!(self.gen(expr));
                try!(self.gen_assign(
                    retrieve_from_load(expr),
                    &node::AST::new(
                        node::ASTKind::BinaryOp(
                            Box::new(expr.clone()),
                            Box::new(node::AST::new(
                                node::ASTKind::Int(1, Bits::Bits32),
                                Pos::new(0, 0),
                            )),
                            node::CBinOps::Add,
                        ),
                        Pos::new(0, 0),
                    ),
                ));
                Ok(before_inc)
            }
            node::CUnaryOps::Dec => {
                let before_dec = try!(self.gen(expr));
                try!(self.gen_assign(
                    retrieve_from_load(expr),
                    &node::AST::new(
                        node::ASTKind::BinaryOp(
                            Box::new(expr.clone()),
                            Box::new(node::AST::new(
                                node::ASTKind::Int(1, Bits::Bits32),
                                Pos::new(0, 0),
                            )),
                            node::CBinOps::Sub,
                        ),
                        Pos::new(0, 0),
                    ),
                ));
                Ok(before_dec)
            }
            _ => Ok((ptr::null_mut(), None)),
        }
    }

    unsafe fn gen_binary_op(
        &mut self,
        lhsast: &node::AST,
        rhsast: &node::AST,
        op: &node::CBinOps,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        match *op {
            // logical operators
            node::CBinOps::LAnd => return self.gen_logand_op(lhsast, rhsast),
            node::CBinOps::LOr => return self.gen_logor_op(lhsast, rhsast),
            // assignment
            node::CBinOps::Assign => {
                return self.gen_assign(retrieve_from_load(lhsast), rhsast);
            }
            _ => {}
        }

        // normal binary operators
        let (lhs, lhsty_w) = try!(self.gen(lhsast));
        let (rhs, rhsty_w) = try!(self.gen(rhsast));

        let lhsty = lhsty_w.unwrap().conversion();
        let rhsty = rhsty_w.unwrap().conversion();

        if matches!(lhsty, Type::Ptr(_)) && matches!(rhsty, Type::Ptr(_)) {
            let castlhs = self.typecast(lhs, LLVMInt64Type());
            let castrhs = self.typecast(rhs, LLVMInt64Type());
            return Ok((
                self.gen_int_binary_op(castlhs, castrhs, op),
                Some(Type::LLong(Sign::Signed)),
            ));
        }

        if let Type::Ptr(elem_ty) = lhsty {
            return self.gen_ptr_binary_op(lhs, rhs, Type::Ptr(elem_ty), op);
        }
        if let Type::Ptr(elem_ty) = rhsty {
            return self.gen_ptr_binary_op(rhs, lhs, Type::Ptr(elem_ty), op);
        }

        let (conv_ty, conv_llvm_ty) = if lhsty.priority() < rhsty.priority() {
            (rhsty.clone(), LLVMTypeOf(rhs))
        } else {
            (lhsty.clone(), LLVMTypeOf(lhs))
        };

        if conv_ty.is_float_ty() {
            let castrhs = self.typecast(rhs, conv_llvm_ty);
            let castlhs = self.typecast(lhs, conv_llvm_ty);
            return Ok((
                self.gen_double_binary_op(castlhs, castrhs, op),
                Some(conv_ty),
            ));
        }

        if conv_ty.is_int_ty() {
            let castrhs = self.typecast(rhs, conv_llvm_ty);
            let castlhs = self.typecast(lhs, conv_llvm_ty);
            return Ok((self.gen_int_binary_op(castlhs, castrhs, op), Some(conv_ty)));
        }

        Err(Error::MsgWithPos(
            "unsupported operation".to_string(),
            lhsast.pos.clone(),
        ))
    }

    // TODO: refine code
    unsafe fn gen_logand_op(
        &mut self,
        lhsast: &node::AST,
        rhsast: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let lhs_val = {
            let val = try!(self.gen(lhsast)).0;
            LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntNE,
                val,
                LLVMConstNull(LLVMTypeOf(val)),
                CString::new("eql").unwrap().as_ptr(),
            )
        };

        let func = self.cur_func.unwrap();

        let bb_then = LLVMAppendBasicBlock(func, CString::new("then").unwrap().as_ptr());
        let bb_merge = LLVMAppendBasicBlock(func, CString::new("merge").unwrap().as_ptr());
        let x = LLVMGetInsertBlock(self.builder);

        LLVMBuildCondBr(self.builder, lhs_val, bb_then, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_then);
        // then block
        let rhs_val = {
            let val = try!(self.gen(rhsast)).0;
            LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntNE,
                val,
                LLVMConstNull(LLVMTypeOf(val)),
                CString::new("eql").unwrap().as_ptr(),
            )
        };
        let y = LLVMGetInsertBlock(self.builder);
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_merge);

        let phi = LLVMBuildPhi(
            self.builder,
            LLVMTypeOf(rhs_val),
            CString::new("logand").unwrap().as_ptr(),
        );
        LLVMAddIncoming(
            phi,
            vec![LLVMConstInt(LLVMInt1Type(), 0, 0)]
                .as_mut_slice()
                .as_mut_ptr(),
            vec![x].as_mut_slice().as_mut_ptr(),
            1,
        );
        LLVMAddIncoming(
            phi,
            vec![rhs_val].as_mut_slice().as_mut_ptr(),
            vec![y].as_mut_slice().as_mut_ptr(),
            1,
        );

        Ok((phi, Some(Type::Int(Sign::Signed))))
    }
    unsafe fn gen_logor_op(
        &mut self,
        lhsast: &node::AST,
        rhsast: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let lhs_val = {
            let val = try!(self.gen(lhsast)).0;
            LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntNE,
                val,
                LLVMConstNull(LLVMTypeOf(val)),
                CString::new("eql").unwrap().as_ptr(),
            )
        };

        let func = self.cur_func.unwrap();

        let bb_then = LLVMAppendBasicBlock(func, CString::new("then").unwrap().as_ptr());
        let bb_merge = LLVMAppendBasicBlock(func, CString::new("merge").unwrap().as_ptr());
        let x = LLVMGetInsertBlock(self.builder);

        LLVMBuildCondBr(self.builder, lhs_val, bb_merge, bb_then);

        LLVMPositionBuilderAtEnd(self.builder, bb_then);
        // then block
        let rhs_val = {
            let val = try!(self.gen(rhsast)).0;
            LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntNE,
                val,
                LLVMConstNull(LLVMTypeOf(val)),
                CString::new("eql").unwrap().as_ptr(),
            )
        };
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_merge);

        let phi = LLVMBuildPhi(
            self.builder,
            LLVMTypeOf(rhs_val),
            CString::new("logor").unwrap().as_ptr(),
        );
        LLVMAddIncoming(
            phi,
            vec![LLVMConstInt(LLVMInt1Type(), 1, 0)]
                .as_mut_slice()
                .as_mut_ptr(),
            vec![x].as_mut_slice().as_mut_ptr(),
            1,
        );
        LLVMAddIncoming(
            phi,
            vec![rhs_val].as_mut_slice().as_mut_ptr(),
            vec![bb_then].as_mut_slice().as_mut_ptr(),
            1,
        );

        Ok((phi, Some(Type::Int(Sign::Signed))))
    }

    unsafe fn gen_assign(
        &mut self,
        lhsast: &node::AST,
        rhsast: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let (dst, ptr_dst_ty_w) = try!(self.gen(lhsast));
        let ptr_dst_ty = ptr_dst_ty_w.unwrap();
        // self.gen returns Ptr(real_type)
        let dst_ty = match ptr_dst_ty.get_elem_ty() {
            Some(ok) => ok,
            None => {
                return Err(Error::MsgWithPos(
                    "gen_assign: ptr_dst_ty must be a pointer to the value's type".to_string(),
                    lhsast.pos.clone(),
                ))
            }
        };
        let (src, _src_ty) = try!(self.gen(rhsast));
        let a = LLVMGetElementType(LLVMTypeOf(dst));
        let casted_src = self.typecast(src, a);
        LLVMBuildStore(self.builder, casted_src, dst);
        Ok((
            LLVMBuildLoad(self.builder, dst, CString::new("load").unwrap().as_ptr()),
            Some((dst_ty).clone()),
        ))
    }

    unsafe fn gen_int_binary_op(
        &mut self,
        lhs: LLVMValueRef,
        rhs: LLVMValueRef,
        op: &node::CBinOps,
    ) -> LLVMValueRef {
        match *op {
            node::CBinOps::Add => LLVMBuildAdd(
                self.builder,
                lhs,
                rhs,
                CString::new("add").unwrap().as_ptr(),
            ),
            node::CBinOps::Sub => LLVMBuildSub(
                self.builder,
                lhs,
                rhs,
                CString::new("sub").unwrap().as_ptr(),
            ),
            node::CBinOps::Mul => LLVMBuildMul(
                self.builder,
                lhs,
                rhs,
                CString::new("mul").unwrap().as_ptr(),
            ),
            node::CBinOps::Div => LLVMBuildSDiv(
                self.builder,
                lhs,
                rhs,
                CString::new("div").unwrap().as_ptr(),
            ),
            node::CBinOps::Rem => LLVMBuildSRem(
                self.builder,
                lhs,
                rhs,
                CString::new("rem").unwrap().as_ptr(),
            ),
            node::CBinOps::Eq => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntEQ,
                lhs,
                rhs,
                CString::new("eql").unwrap().as_ptr(),
            ),
            node::CBinOps::Ne => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntNE,
                lhs,
                rhs,
                CString::new("ne").unwrap().as_ptr(),
            ),
            node::CBinOps::Lt => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntSLT,
                lhs,
                rhs,
                CString::new("lt").unwrap().as_ptr(),
            ),
            node::CBinOps::Gt => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntSGT,
                lhs,
                rhs,
                CString::new("gt").unwrap().as_ptr(),
            ),
            node::CBinOps::Le => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntSLE,
                lhs,
                rhs,
                CString::new("le").unwrap().as_ptr(),
            ),
            node::CBinOps::Ge => LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntSGE,
                lhs,
                rhs,
                CString::new("ge").unwrap().as_ptr(),
            ),
            node::CBinOps::Shl => LLVMBuildShl(
                self.builder,
                lhs,
                rhs,
                CString::new("shl").unwrap().as_ptr(),
            ),
            node::CBinOps::Shr => LLVMBuildAShr(
                self.builder,
                lhs,
                rhs,
                CString::new("shr").unwrap().as_ptr(),
            ),
            node::CBinOps::And => LLVMBuildAnd(
                self.builder,
                lhs,
                rhs,
                CString::new("and").unwrap().as_ptr(),
            ),
            node::CBinOps::Or => {
                LLVMBuildOr(self.builder, lhs, rhs, CString::new("or").unwrap().as_ptr())
            }
            node::CBinOps::Xor => LLVMBuildXor(
                self.builder,
                lhs,
                rhs,
                CString::new("xor").unwrap().as_ptr(),
            ),
            node::CBinOps::Comma => rhs,
            _ => panic!(),
        }
    }

    unsafe fn gen_ptr_binary_op(
        &mut self,
        lhs: LLVMValueRef,
        rhs: LLVMValueRef,
        ty: Type,
        op: &node::CBinOps,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let mut numidx = vec![
            match *op {
                node::CBinOps::Add => rhs,
                node::CBinOps::Sub => LLVMBuildSub(
                    self.builder,
                    try!(self.make_int(0, &Bits::Bits32, false)).0,
                    rhs,
                    CString::new("sub").unwrap().as_ptr(),
                ),
                _ => rhs,
            },
        ];
        Ok((
            LLVMBuildGEP(
                self.builder,
                lhs,
                numidx.as_mut_slice().as_mut_ptr(),
                1,
                CString::new("add").unwrap().as_ptr(),
            ),
            Some(ty),
        ))
    }

    unsafe fn gen_double_binary_op(
        &mut self,
        lhs: LLVMValueRef,
        rhs: LLVMValueRef,
        op: &node::CBinOps,
    ) -> LLVMValueRef {
        match *op {
            node::CBinOps::Add => LLVMBuildFAdd(
                self.builder,
                lhs,
                rhs,
                CString::new("fadd").unwrap().as_ptr(),
            ),
            node::CBinOps::Sub => LLVMBuildFSub(
                self.builder,
                lhs,
                rhs,
                CString::new("fsub").unwrap().as_ptr(),
            ),
            node::CBinOps::Mul => LLVMBuildFMul(
                self.builder,
                lhs,
                rhs,
                CString::new("fmul").unwrap().as_ptr(),
            ),
            node::CBinOps::Div => LLVMBuildFDiv(
                self.builder,
                lhs,
                rhs,
                CString::new("fdiv").unwrap().as_ptr(),
            ),
            node::CBinOps::Eq => LLVMBuildFCmp(
                self.builder,
                llvm::LLVMRealPredicate::LLVMRealOEQ,
                lhs,
                rhs,
                CString::new("feql").unwrap().as_ptr(),
            ),
            node::CBinOps::Ne => LLVMBuildFCmp(
                self.builder,
                llvm::LLVMRealPredicate::LLVMRealONE,
                lhs,
                rhs,
                CString::new("fne").unwrap().as_ptr(),
            ),
            node::CBinOps::Lt => LLVMBuildFCmp(
                self.builder,
                llvm::LLVMRealPredicate::LLVMRealOLT,
                lhs,
                rhs,
                CString::new("flt").unwrap().as_ptr(),
            ),
            node::CBinOps::Gt => LLVMBuildFCmp(
                self.builder,
                llvm::LLVMRealPredicate::LLVMRealOGT,
                lhs,
                rhs,
                CString::new("fgt").unwrap().as_ptr(),
            ),
            node::CBinOps::Le => LLVMBuildFCmp(
                self.builder,
                llvm::LLVMRealPredicate::LLVMRealOLE,
                lhs,
                rhs,
                CString::new("fle").unwrap().as_ptr(),
            ),
            node::CBinOps::Ge => LLVMBuildFCmp(
                self.builder,
                llvm::LLVMRealPredicate::LLVMRealOGE,
                lhs,
                rhs,
                CString::new("fge").unwrap().as_ptr(),
            ),
            _ => ptr::null_mut(),
        }
    }

    unsafe fn gen_ternary_op(
        &mut self,
        cond: &node::AST,
        then_expr: &node::AST,
        else_expr: &node::AST,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let cond_val = {
            let val = try!(self.gen(cond)).0;
            LLVMBuildICmp(
                self.builder,
                llvm::LLVMIntPredicate::LLVMIntNE,
                val,
                LLVMConstNull(LLVMTypeOf(val)),
                CString::new("eql").unwrap().as_ptr(),
            )
        };

        let func = self.cur_func.unwrap();

        let bb_then = LLVMAppendBasicBlock(func, CString::new("then").unwrap().as_ptr());
        let bb_else = LLVMAppendBasicBlock(func, CString::new("else").unwrap().as_ptr());
        let bb_merge = LLVMAppendBasicBlock(func, CString::new("merge").unwrap().as_ptr());

        LLVMBuildCondBr(self.builder, cond_val, bb_then, bb_else);

        LLVMPositionBuilderAtEnd(self.builder, bb_then);
        // then block
        let (then_val, then_ty) = try!(self.gen(then_expr));
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_else);
        // else block
        let (else_val, else_ty) = try!(self.gen(else_expr));
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_merge);

        if matches!(then_ty.clone().unwrap(), Type::Void) || matches!(else_ty.unwrap(), Type::Void)
        {
            return Ok((ptr::null_mut(), None));
        }

        let phi = LLVMBuildPhi(
            self.builder,
            LLVMTypeOf(then_val),
            CString::new("ternary_phi").unwrap().as_ptr(),
        );
        LLVMAddIncoming(
            phi,
            vec![then_val].as_mut_slice().as_mut_ptr(),
            vec![bb_then].as_mut_slice().as_mut_ptr(),
            1,
        );
        LLVMAddIncoming(
            phi,
            vec![else_val].as_mut_slice().as_mut_ptr(),
            vec![bb_else].as_mut_slice().as_mut_ptr(),
            1,
        );

        Ok((phi, then_ty))
    }

    unsafe fn gen_struct_field(
        &mut self,
        expr: &node::AST,
        field_name: String,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let (val, ptr_ty) = try!(self.get_struct_field_val(retrieve_from_load(expr), field_name));
        Ok((val, ptr_ty))
    }
    unsafe fn get_struct_field_val(
        &mut self,
        expr: &node::AST,
        field_name: String,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let (strct, ptr_ty_w) = try!(self.gen(expr));
        let ptr_ty = ptr_ty_w.unwrap();
        let ty = ptr_ty
            .get_elem_ty()
            .or_else(|| panic!("gen_assign: ptr_dst_ty must be a pointer to the value's type"))
            .unwrap();
        let strct_name = ty.get_name();
        assert!(strct_name.is_some());

        let ref rectype = self.llvm_struct_map
            .get(strct_name.unwrap().as_str())
            .unwrap();
        let idx = *rectype.field_pos.get(field_name.as_str()).unwrap();
        if rectype.is_struct {
            Ok((
                LLVMBuildStructGEP(
                    self.builder,
                    strct,
                    idx,
                    CString::new("structref").unwrap().as_ptr(),
                ),
                Some(Type::Ptr(Box::new(
                    rectype.field_types[idx as usize].clone(),
                ))),
            ))
        } else {
            let llvm_idx_ty = rectype.field_llvm_types[idx as usize];
            Ok((
                self.typecast(strct, LLVMPointerType(llvm_idx_ty, 0)),
                Some(Type::Ptr(Box::new(
                    rectype.field_types[idx as usize].clone(),
                ))),
            ))
        }
    }

    unsafe fn gen_type_cast(
        &mut self,
        expr: &node::AST,
        ty: &Type,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
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

    unsafe fn gen_var(&mut self, name: &String) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let varinfo_w = self.lookup_var(name.as_str());
        match varinfo_w {
            Some(varinfo) => Ok((varinfo.llvm_val, Some(Type::Ptr(Box::new(varinfo.ty))))),
            None => panic!("undeclared variable"),
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

    unsafe fn gen_load(&mut self, var: &node::AST) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let (val, ty) = try!(self.gen(var));

        if let Some(Type::Ptr(ref elem_ty)) = ty {
            match **elem_ty {
                Type::Func(_, _, _) => return Ok((val, Some(Type::Ptr((*elem_ty).clone())))),
                Type::Array(ref ary_elemty, _) => {
                    return Ok((
                        LLVMBuildGEP(
                            self.builder,
                            val,
                            vec![
                                try!(self.make_int(0, &Bits::Bits32, false)).0,
                                try!(self.make_int(0, &Bits::Bits32, false)).0,
                            ].as_mut_slice()
                                .as_mut_ptr(),
                            2,
                            CString::new("gep").unwrap().as_ptr(),
                        ),
                        Some(Type::Ptr(Box::new((**ary_elemty).clone()))),
                    ));
                }
                _ => {
                    return Ok((
                        LLVMBuildLoad(self.builder, val, CString::new("var").unwrap().as_ptr()),
                        Some((**elem_ty).clone()),
                    ));
                }
            }
        } else {
            panic!();
        }
    }

    unsafe fn gen_func_call(
        &mut self,
        ast: &node::AST,
        args: &Vec<node::AST>,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        // there's a possibility that the types of args are not the same as the types of params.
        // so the args before implicit type casting are called 'maybe correct args'.
        let mut maybe_correct_args_val = vec![];
        for arg in &*args {
            maybe_correct_args_val.push(try!(self.gen(arg)).0);
        }
        let args_len = args.len();

        let func = match retrieve_from_load(ast) {
            &node::AST {
                kind: node::ASTKind::Variable(_, ref name),
                pos: _,
            } => {
                if let Some(varinfo) = self.lookup_var(name) {
                    varinfo
                } else {
                    return Err(Error::MsgWithPos(
                        format!("gen_func_call: not found function '{}'", name),
                        ast.pos.clone(),
                    ));
                }
            }
            x => {
                let (val, ty) = try!(self.gen(&x));
                VarInfo::new(ty.unwrap(), LLVMTypeOf(val), val)
            }
        };

        fn decompose(ty: Type) -> Option<(Box<Type>, Vec<Type>, bool)> {
            match ty {
                Type::Func(ret_ty, params_types, is_vararg) => {
                    Some((ret_ty, params_types, is_vararg))
                }
                // call function pointer
                Type::Ptr(elemty) => decompose((*elemty).clone()),
                _ => None,
            }
        }

        let (func_ret_ty, func_params_types, func_is_vararg) = match decompose(func.ty) {
            Some((func_ret_ty, func_params_types, func_is_vararg)) => {
                (func_ret_ty, func_params_types, func_is_vararg)
            }
            None => {
                return Err(Error::MsgWithPos(
                    format!("gen_func_call: function type doesn't match"),
                    ast.pos.clone(),
                ))
            }
        };

        let (llvm_func, llvm_functy) = match LLVMGetTypeKind(func.llvm_ty) {
            llvm::LLVMTypeKind::LLVMPointerTypeKind => (
                LLVMBuildLoad(
                    self.builder,
                    func.llvm_val,
                    CString::new("load").unwrap().as_ptr(),
                ),
                LLVMGetElementType(func.llvm_ty),
            ),
            _ => (func.llvm_val, func.llvm_ty),
        };

        let params_count = func_params_types.len();
        let mut args_val = Vec::new();
        let ptr_params_types = (&mut Vec::with_capacity(params_count)).as_mut_ptr();
        LLVMGetParamTypes(llvm_functy, ptr_params_types);
        let llvm_params_types = Vec::from_raw_parts(ptr_params_types, params_count, 0);

        if !func_is_vararg && params_count < args_len {
            return Err(Error::MsgWithPos(
                "too many arguments".to_string(),
                ast.pos.clone(),
            ));
        }
        if !func_is_vararg && params_count > args_len {
            return Err(Error::MsgWithPos(
                "too little arguments".to_string(),
                ast.pos.clone(),
            ));
        }
        // do implicit type casting
        for i in 0..args_len {
            args_val.push(if params_count <= i {
                maybe_correct_args_val[i]
            } else {
                self.typecast(maybe_correct_args_val[i], llvm_params_types[i])
            })
        }

        Ok((
            LLVMBuildCall(
                self.builder,
                llvm_func,
                args_val.as_mut_slice().as_mut_ptr(),
                args_len as u32,
                CString::new("").unwrap().as_ptr(),
            ),
            Some((*func_ret_ty).clone()),
        ))
    }
    unsafe fn gen_continue(&mut self) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let continue_bb = if let Some(l) = self.continue_labels.back() {
            *l
        } else {
            return Err(Error::Msg(
                "continue error (maybe not in loop or switch stmt)".to_string(),
            ));
        };
        LLVMBuildBr(self.builder, continue_bb);
        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_break(&mut self) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let break_bb = if let Some(l) = self.break_labels.back() {
            *l
        } else {
            return Err(Error::Msg(
                "break error (maybe not in loop or switch stmt)".to_string(),
            ));
        };
        LLVMBuildBr(self.builder, break_bb);
        Ok((ptr::null_mut(), None))
    }

    unsafe fn gen_return(&mut self, ret_ast: &node::AST) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let (ret_val, _) = try!(self.gen(ret_ast));
        Ok((
            LLVMBuildRet(
                self.builder,
                self.typecast(
                    ret_val,
                    LLVMGetReturnType(LLVMGetElementType(LLVMTypeOf(self.cur_func.unwrap()))),
                ),
            ),
            None,
        ))
    }

    pub unsafe fn make_int(
        &mut self,
        n: u64,
        bits: &Bits,
        is_unsigned: bool,
    ) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        let ty = match *bits {
            Bits::Bits8 => LLVMInt8Type(),
            Bits::Bits16 => LLVMInt16Type(),
            Bits::Bits32 => LLVMInt32Type(),
            Bits::Bits64 => LLVMInt64Type(),
        };
        Ok((
            LLVMConstInt(ty, n, if is_unsigned { 1 } else { 0 }),
            Some(Type::Int(Sign::Signed)),
        ))
    }
    pub unsafe fn make_char(&mut self, n: i32) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        Ok((
            LLVMConstInt(LLVMInt8Type(), n as u64, 0),
            Some(Type::Char(Sign::Signed)),
        ))
    }
    pub unsafe fn make_float(&mut self, f: f64) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        Ok((LLVMConstReal(LLVMFloatType(), f), Some(Type::Float)))
    }
    pub unsafe fn make_double(&mut self, f: f64) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        Ok((LLVMConstReal(LLVMDoubleType(), f), Some(Type::Double)))
    }
    pub unsafe fn make_const_str(&mut self, s: &String) -> CodegenR<(LLVMValueRef, Option<Type>)> {
        Ok((
            LLVMBuildGlobalStringPtr(
                self.builder,
                CString::new(s.as_str()).unwrap().as_ptr(),
                CString::new("str").unwrap().as_ptr(),
            ),
            Some(Type::Ptr(Box::new(Type::Char(Sign::Signed)))),
        ))
    }

    // Functions Related to Types
    pub unsafe fn typecast(&self, val: LLVMValueRef, to: LLVMTypeRef) -> LLVMValueRef {
        let v_ty = LLVMTypeOf(val);
        let inst_name = CString::new("").unwrap().as_ptr();

        if matches!(LLVMGetTypeKind(to), llvm::LLVMTypeKind::LLVMVoidTypeKind) {
            return val;
        }

        match LLVMGetTypeKind(v_ty) {
            llvm::LLVMTypeKind::LLVMIntegerTypeKind => match LLVMGetTypeKind(to) {
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
            },
            llvm::LLVMTypeKind::LLVMDoubleTypeKind | llvm::LLVMTypeKind::LLVMFloatTypeKind => {
                return LLVMBuildFPToSI(self.builder, val, to, inst_name);
            }
            llvm::LLVMTypeKind::LLVMVoidTypeKind => return val,
            llvm::LLVMTypeKind::LLVMPointerTypeKind => match LLVMGetTypeKind(to) {
                llvm::LLVMTypeKind::LLVMIntegerTypeKind => {
                    return LLVMBuildPtrToInt(self.builder, val, to, inst_name);
                }
                _ => {}
            },
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
            &Type::Long(_) => LLVMInt64Type(),
            &Type::LLong(_) => LLVMInt64Type(),
            &Type::Float => LLVMFloatType(),
            &Type::Double => LLVMDoubleType(),
            &Type::Ptr(ref elemty) => LLVMPointerType(
                || -> LLVMTypeRef {
                    let elemty = self.type_to_llvmty(&**elemty);
                    match LLVMGetTypeKind(elemty) {
                        llvm::LLVMTypeKind::LLVMVoidTypeKind => LLVMInt8Type(),
                        _ => elemty,
                    }
                }(),
                0,
            ),
            &Type::Array(ref elemty, ref size) => {
                LLVMArrayType(self.type_to_llvmty(&**elemty), *size as u32)
            }
            &Type::Func(ref ret_type, ref param_types, ref is_vararg) => LLVMFunctionType(
                self.type_to_llvmty(&**ret_type),
                || -> *mut LLVMTypeRef {
                    let mut param_llvm_types: Vec<LLVMTypeRef> = Vec::new();
                    for param_type in &*param_types {
                        param_llvm_types.push(self.type_to_llvmty(&param_type));
                    }
                    param_llvm_types.as_mut_slice().as_mut_ptr()
                }(),
                (*param_types).len() as u32,
                if *is_vararg { 1 } else { 0 },
            ),
            &Type::Struct(ref name, ref fields) => self.make_struct(name, fields),
            &Type::Union(ref name, ref fields, ref max_size_field_pos) => {
                self.make_union(name, fields, *max_size_field_pos)
            }
            &Type::Enum => LLVMInt32Type(),
        }
    }
    unsafe fn make_rectype_base(
        &mut self,
        name: &String,
        fields: &Vec<node::AST>,
        fields_names_map: &mut HashMap<String, u32>,
        fields_llvm_types: &mut Vec<LLVMTypeRef>,
        fields_types: &mut Vec<Type>,
        is_struct: bool,
        // returns (the rectype already declared, LLVMStructType)
    ) -> (bool, LLVMTypeRef) {
        let new_struct: LLVMTypeRef = {
            let struct_ = self.llvm_struct_map.get(name);
            if let Some(ref rectype) = struct_ {
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

        self.llvm_struct_map.insert(
            name.to_string(),
            RectypeInfo::new(
                HashMap::new(),
                Vec::new(),
                Vec::new(),
                new_struct,
                is_struct,
            ),
        );

        for (i, field) in fields.iter().enumerate() {
            if let node::ASTKind::VariableDecl(ref ty, ref name, ref _sclass, ref _init) =
                field.kind
            {
                fields_llvm_types.push(self.type_to_llvmty(ty));
                fields_types.push(ty.clone());
                fields_names_map.insert(name.to_string(), i as u32);
            } else {
                panic!("never reach here");
            }
        }
        (false, new_struct)
    }
    unsafe fn make_struct(&mut self, name: &String, fields: &Vec<node::AST>) -> LLVMTypeRef {
        let mut fields_names_map: HashMap<String, u32> = HashMap::new();
        let mut fields_llvm_types: Vec<LLVMTypeRef> = Vec::new();
        let mut fields_types: Vec<Type> = Vec::new();
        let (exist, new_struct) = self.make_rectype_base(
            name,
            fields,
            &mut fields_names_map,
            &mut fields_llvm_types,
            &mut fields_types,
            true,
        );
        if exist {
            return new_struct;
        }
        LLVMStructSetBody(
            new_struct,
            fields_llvm_types.as_mut_slice().as_mut_ptr(),
            fields_llvm_types.len() as u32,
            0,
        );
        self.llvm_struct_map.insert(
            name.to_string(),
            RectypeInfo::new(
                fields_names_map,
                fields_types,
                fields_llvm_types,
                new_struct,
                true,
            ),
        );
        new_struct
    }
    unsafe fn make_union(
        &mut self,
        name: &String,
        fields: &Vec<node::AST>,
        max_size_field_pos: usize,
    ) -> LLVMTypeRef {
        let mut fields_names_map: HashMap<String, u32> = HashMap::new();
        let mut fields_llvm_types: Vec<LLVMTypeRef> = Vec::new();
        let mut fields_types: Vec<Type> = Vec::new();
        let (exist, new_struct) = self.make_rectype_base(
            name,
            fields,
            &mut fields_names_map,
            &mut fields_llvm_types,
            &mut fields_types,
            true,
        );
        if exist {
            return new_struct;
        }
        // size of an union is the same as the biggest type in the union
        LLVMStructSetBody(
            new_struct,
            vec![fields_llvm_types[max_size_field_pos]]
                .as_mut_slice()
                .as_mut_ptr(),
            1,
            0,
        );
        self.llvm_struct_map.insert(
            name.to_string(),
            RectypeInfo::new(
                fields_names_map,
                fields_types,
                fields_llvm_types,
                new_struct,
                false,
            ),
        );
        new_struct
    }
}
