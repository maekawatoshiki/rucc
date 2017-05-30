use lexer::{Lexer, Token, TokenKind};
use node::AST;
use node;
use error;
use types::{Type, Sign};

use std::fs::OpenOptions;
use std::io::prelude::*;
use std::{str, u32};
use std::rc::Rc;
use std::collections::{HashMap, VecDeque, hash_map};

extern crate llvm_sys as llvm;

#[derive(PartialEq, Debug, Clone)]
enum StorageClass {
    Typedef,
    Extern,
    Static,
    Auto,
    Register,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    env: VecDeque<HashMap<String, AST>>,
    tags: VecDeque<HashMap<String, Type>>,
}

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> Parser<'a> {
        let mut env = VecDeque::new();
        let mut tags = VecDeque::new();
        env.push_back(HashMap::new());
        tags.push_back(HashMap::new());

        Parser {
            lexer: lexer,
            env: env,
            tags: tags,
        }
    }
    pub fn run_file(filename: String) -> Vec<AST> {
        let mut nodes: Vec<AST> = Vec::new();
        let mut file = OpenOptions::new()
            .read(true)
            .open(filename.to_string())
            .unwrap();
        let mut s = String::new();
        file.read_to_string(&mut s);
        let mut lexer = Lexer::new(filename.to_string(), s.as_str());
        loop {
            let tok = lexer.get();
            match tok {
                Some(t) => {
                    println!("t:{}{}", if t.space { " " } else { "" }, t.val);
                }
                None => break,
            }
        }

        // Debug: (parsing again is big cost?)
        lexer = Lexer::new(filename.to_string(), s.as_str());
        Parser::new(lexer).run(&mut nodes);
        nodes
    }
    pub fn run_string(input: String) -> Vec<AST> {
        let lexer = Lexer::new("__input__".to_string(), input.as_str());
        let mut parser = Parser::new(lexer);
        let mut nodes: Vec<AST> = Vec::new();
        parser.run(&mut nodes);
        nodes
    }
    pub fn run(&mut self, node: &mut Vec<AST>) {
        loop {
            if self.lexer.peek().is_none() {
                break;
            }
            self.read_toplevel(node);
        }
    }
    pub fn run_as_expr(&mut self) -> AST {
        self.read_expr()
    }

    fn read_toplevel(&mut self, ast: &mut Vec<AST>) {
        if self.is_function_def() {
            ast.push(self.read_func_def());
        } else {
            self.read_decl(ast);
        }
    }
    fn read_func_def(&mut self) -> AST {
        let localenv = (*self.env.back().unwrap()).clone();
        let localtags = (*self.tags.back().unwrap()).clone();
        self.env.push_back(localenv);
        self.tags.push_back(localtags);

        let retty = self.read_type_spec().0;
        let (functy, name, param_names) = self.read_declarator(retty);
        println!("functy: {:?}", functy);

        self.lexer.expect_skip("{");
        let body = self.read_func_body(&functy);
        self.env.pop_back();
        self.tags.pop_back();
        AST::FuncDef(functy,
                     if param_names.is_none() {
                         Vec::new()
                     } else {
                         param_names.unwrap()
                     },
                     name,
                     Rc::new(body))
    }
    fn read_func_body(&mut self, _functy: &Type) -> AST {
        self.read_compound_stmt()
    }
    fn read_compound_stmt(&mut self) -> AST {
        let mut stmts: Vec<AST> = Vec::new();
        loop {
            if self.lexer.skip("}") {
                break;
            }

            let peek_tok = self.lexer.peek_e();
            if self.is_type(&peek_tok) {
                // variable declaration
                self.read_decl(&mut stmts);
            } else {
                stmts.push(self.read_stmt());
            }
        }
        AST::Block(stmts)
    }
    fn read_stmt(&mut self) -> AST {
        let tok = self.lexer.get_e();
        match tok.val.as_str() {
            "{" => return self.read_compound_stmt(),
            "if" => return self.read_if_stmt(),
            "for" => return self.read_for_stmt(),
            "while" => return self.read_while_stmt(),
            "return" => return self.read_return_stmt(),
            _ => {}
        }
        self.lexer.unget(tok);
        let expr = self.read_expr();
        self.lexer.expect_skip(";");
        expr
    }
    fn read_if_stmt(&mut self) -> AST {
        self.lexer.expect_skip("(");
        let cond = self.read_expr();
        self.lexer.expect_skip(")");
        let then_stmt = Rc::new(self.read_stmt());
        let else_stmt = if self.lexer.skip("else") {
            Rc::new(self.read_stmt())
        } else {
            Rc::new(AST::Block(Vec::new()))
        };
        AST::If(Rc::new(cond), then_stmt, else_stmt)
    }
    fn read_for_stmt(&mut self) -> AST {
        self.lexer.expect_skip("(");
        let init = self.read_opt_decl_or_stmt();
        // TODO: make read_expr returns Option<AST>.
        //       when cur tok is ';', returns None.
        let cond = self.read_opt_expr();
        self.lexer.expect_skip(";");
        let step = self.read_opt_expr();
        self.lexer.expect_skip(")");
        let body = self.read_stmt();
        AST::For(Rc::new(init), Rc::new(cond), Rc::new(step), Rc::new(body))
    }
    fn read_while_stmt(&mut self) -> AST {
        self.lexer.expect_skip("(");
        let cond = self.read_expr();
        self.lexer.expect_skip(")");
        let body = self.read_stmt();
        AST::While(Rc::new(cond), Rc::new(body))
    }
    fn read_return_stmt(&mut self) -> AST {
        if self.lexer.skip(";") {
            AST::Return(None)
        } else {
            let retval = Some(Rc::new(self.read_expr()));
            self.lexer.expect_skip(";");
            AST::Return(retval)
        }
    }
    fn is_function_def(&mut self) -> bool {
        let mut buf: Vec<Token> = Vec::new();
        let mut is_funcdef = false;

        loop {
            let mut tok = self.lexer.get().unwrap();
            buf.push(tok.clone());

            if tok.val == ";" {
                break;
            }

            if self.is_type(&tok) {
                continue;
            }

            if tok.val == "(" {
                self.skip_brackets(&mut buf);
                continue;
            }

            if tok.kind != TokenKind::Identifier {
                continue;
            }

            if self.lexer.peek().unwrap().val != "(" {
                continue;
            }

            buf.push(self.lexer.get().unwrap());
            self.skip_brackets(&mut buf);

            tok = self.lexer.peek().unwrap();
            is_funcdef = tok.val.as_str() == "{";
            break;
        }

        self.lexer.unget_all(buf);
        is_funcdef
    }
    fn skip_brackets(&mut self, buf: &mut Vec<Token>) {
        loop {
            let tok = self.lexer.get().unwrap();
            buf.push(tok.clone());

            match tok.val.as_str() {
                "(" => self.skip_brackets(buf),
                ")" => break,
                _ => {}
            }
        }
    }

    fn get_typedef(&mut self, name: &str) -> Option<Type> {
        let env = self.env.back_mut().unwrap();
        match env.get(name) {
            Some(ast) => {
                Some(match ast {
                         &AST::Typedef(ref from, ref _to) => (*from).clone(),
                         _ => {
                             error::error_exit(self.lexer.cur_line,
                                               format!("not found type '{}'", name).as_str())
                         }
                     })
            } 
            None => None,
        }
    }
    fn is_type(&mut self, token: &Token) -> bool {
        if token.kind != TokenKind::Identifier {
            return false;
        }
        let name = token.val.as_str();
        match name {
            "void" | "signed" | "unsigned" | "char" | "int" | "short" | "long" | "float" |
            "double" | "struct" | "union" | "extern" | "const" | "volatile" => true,
            _ => self.env.back_mut().unwrap().contains_key(name),
        }
    }
    fn read_decl_init(&mut self, ty: &mut Type) -> AST {
        // TODO: implement for like 'int a[] = {...}, char *s="str";'
        if self.lexer.skip("{") || self.lexer.peek_e().kind == TokenKind::String {
            // self.read_
            self.read_initializer_list(ty)
        } else {
            self.read_assign()
        }
    }
    fn read_initializer_list(&mut self, ty: &mut Type) -> AST {
        match ty {
            &mut Type::Array(_, _) => self.read_array_initializer(ty),
            _ => self.read_assign(),
        }
    }
    fn read_array_initializer(&mut self, ty: &mut Type) -> AST {
        if let &mut Type::Array(ref mut _elem_ty, ref mut len) = ty {
            let is_flexible = *len < 0;
            let mut elems = Vec::new();
            let mut count = 0;
            loop {
                if self.lexer.skip("}") {
                    break;
                }
                let elem = self.read_assign();
                elems.push(elem);
                self.lexer.skip(",");
                count += 1;
            }
            if is_flexible {
                *len = count;
            }
            AST::ConstArray(elems)
        } else {
            error::error_exit(self.lexer.cur_line, "impossible");
        }
        // self.read_assign()
    }
    fn skip_type_qualifiers(&mut self) {
        while self.lexer.skip("const") || self.lexer.skip("volatile") ||
              self.lexer.skip("restrict") {}
    }
    fn read_decl(&mut self, ast: &mut Vec<AST>) {
        let (basety, sclass) = self.read_type_spec();
        let is_typedef = sclass.is_some() && sclass.unwrap() == StorageClass::Typedef;
        if self.lexer.skip(";") {
            return;
        }

        loop {
            let (mut ty, name, _) = self.read_declarator(basety.clone());

            if is_typedef {
                let typedef = AST::Typedef(basety, name.to_string());
                self.env.back_mut().unwrap().insert(name, typedef);
                return;
            }

            let init = if self.lexer.skip("=") {
                Some(Rc::new(self.read_decl_init(&mut ty)))
            } else {
                None
            };
            ast.push(AST::VariableDecl(ty, name, init));

            if self.lexer.skip(";") {
                return;
            }
            self.lexer.expect_skip(",");
        }
    }
    fn read_opt_decl_or_stmt(&mut self) -> AST {
        if self.lexer.skip(";") {
            return AST::Compound(Vec::new());
        }

        let peek_tok = self.lexer.peek_e();
        if self.is_type(&peek_tok) {
            // variable declaration
            let mut stmts = Vec::new();
            self.read_decl(&mut stmts);
            AST::Compound(stmts)
        } else {
            self.read_stmt()
        }
    }
    // returns (declarator type, name, params{for function})
    fn read_declarator(&mut self, basety: Type) -> (Type, String, Option<Vec<String>>) {
        if self.lexer.skip("(") {
            let peek_tok = self.lexer.peek_e();
            if self.is_type(&peek_tok) {
                let (ty, params) = self.read_declarator_func(basety);
                return (ty, "".to_string(), params);
            }

            // TODO: HUH? MAKES NO SENSE!!
            let mut buf: Vec<Token> = Vec::new();
            while !self.lexer.skip(")") {
                buf.push(self.lexer.get().unwrap());
            }
            let t = self.read_declarator_tail(basety);
            self.lexer.unget_all(buf);
            return self.read_declarator(t.0);
        }

        if self.lexer.skip("*") {
            self.skip_type_qualifiers();
            return self.read_declarator(Type::Ptr(Rc::new(basety)));
        }

        let tok = self.lexer.get().unwrap();

        if tok.kind == TokenKind::Identifier {
            let name = tok.val;
            let (ty, params) = self.read_declarator_tail(basety);
            return (ty, name, params);
        }

        self.lexer.unget(tok);
        let (ty, params) = self.read_declarator_tail(basety);
        (ty, "".to_string(), params)
    }
    fn read_declarator_tail(&mut self, basety: Type) -> (Type, Option<Vec<String>>) {
        if self.lexer.skip("[") {
            return (self.read_declarator_array(basety), None);
        }
        if self.lexer.skip("(") {
            return self.read_declarator_func(basety);
        }
        (basety, None)
    }

    fn read_declarator_array(&mut self, basety: Type) -> Type {
        let len: i32;
        if self.lexer.skip("]") {
            len = -1;
        } else {
            len = self.read_expr().eval_constexpr();
            self.lexer.expect_skip("]");
        }
        let ty = self.read_declarator_tail(basety).0;
        Type::Array(Rc::new(ty), len)
    }
    fn read_declarator_func(&mut self, retty: Type) -> (Type, Option<Vec<String>>) {
        if self.lexer.skip("void") {
            self.lexer.expect_skip(")");
            return (Type::Func(Rc::new(retty), Vec::new(), false), None);
        }
        if self.lexer.skip(")") {
            return (Type::Func(Rc::new(retty), Vec::new(), false), None);
        }

        let (paramtypes, paramnames, vararg) = self.read_declarator_params();
        (Type::Func(Rc::new(retty), paramtypes.clone(), vararg), Some(paramnames))
    }
    // returns (param types, param names, vararg?)
    fn read_declarator_params(&mut self) -> (Vec<Type>, Vec<String>, bool) {
        let mut paramtypes: Vec<Type> = Vec::new();
        let mut paramnames: Vec<String> = Vec::new();
        loop {
            if self.lexer.skip("...") {
                if paramtypes.len() == 0 {
                    error::error_exit(self.lexer.cur_line,
                                      "at least one param is required before '...'");
                }
                self.lexer.expect_skip(")");
                return (paramtypes, paramnames, true);
            }

            let (ty, name) = self.read_func_param();
            paramtypes.push(ty);
            paramnames.push(name);
            if self.lexer.skip(")") {
                return (paramtypes, paramnames, false);
            }
            self.lexer.expect_skip(",");
        }
    }
    fn read_func_param(&mut self) -> (Type, String) {
        let basety = self.read_type_spec().0;
        let (ty, name, _) = self.read_declarator(basety);
        match ty {
            Type::Array(subst, _) => return (Type::Ptr(subst), name),
            Type::Func(_, _, _) => return (Type::Ptr(Rc::new(ty)), name),
            _ => {}
        }
        (ty, name)
    }
    fn read_type_spec(&mut self) -> (Type, Option<StorageClass>) {
        #[derive(PartialEq, Debug, Clone)]
        enum Size {
            Short,
            Normal,
            Long,
            LLong,
        };
        #[derive(PartialEq, Debug, Clone)]
        enum PrimitiveType {
            Void,
            Char,
            Int,
            Float,
            Double,
        };

        let mut kind: Option<PrimitiveType> = None;
        let mut sign: Option<Sign> = None;
        let mut size = Size::Normal;
        let mut sclass: Option<StorageClass> = None;
        let mut userty: Option<Type> = None;

        let err_kind = |lexer: &Lexer, kind: Option<PrimitiveType>| if kind.is_some() {
            error::error_exit(lexer.cur_line, "type mismatch");
        };
        let err_sign = |lexer: &Lexer, sign: Option<Sign>| if sign.is_some() {
            error::error_exit(lexer.cur_line, "type mismatch");
        };

        // let mut env = ENV_MAP.lock().unwrap();

        loop {
            let tok = self.lexer
                .get()
                .or_else(|| error::error_exit(self.lexer.cur_line, "expect types but reach EOF"))
                .unwrap();

            if kind.is_none() && tok.kind == TokenKind::Identifier {
                let maybe_userty_name = tok.val.as_str();
                let maybe_userty = self.get_typedef(maybe_userty_name);
                if maybe_userty.is_some() {
                    return (maybe_userty.unwrap(), sclass);
                }
            }

            match tok.val.as_str() {
                "typedef" => sclass = Some(StorageClass::Typedef),
                "extern" => sclass = Some(StorageClass::Extern),
                "static" => sclass = Some(StorageClass::Static),
                "auto" => sclass = Some(StorageClass::Auto),
                "register" => sclass = Some(StorageClass::Register),
                "const" | "volatile" | "inline" | "noreturn" => {}
                "void" => {
                    err_kind(&self.lexer, kind);
                    kind = Some(PrimitiveType::Void);
                }
                "char" => {
                    err_kind(&self.lexer, kind);
                    kind = Some(PrimitiveType::Char);
                }
                "int" => {
                    err_kind(&self.lexer, kind);
                    kind = Some(PrimitiveType::Int);
                }
                "float" => {
                    err_kind(&self.lexer, kind);
                    kind = Some(PrimitiveType::Float);
                }
                "double" => {
                    err_kind(&self.lexer, kind);
                    kind = Some(PrimitiveType::Double);
                }
                "signed" => {
                    err_sign(&self.lexer, sign);
                    sign = Some(Sign::Signed);
                }
                "unsigned" => {
                    err_sign(&self.lexer, sign);
                    sign = Some(Sign::Unsigned);
                }
                "short" => size = Size::Short,
                "long" => {
                    if size == Size::Normal {
                        size = Size::Long;
                    } else if size == Size::Long {
                        size = Size::LLong;
                    }
                }
                "struct" => userty = Some(self.read_struct_def()),
                _ => {
                    self.lexer.unget(tok);
                    break;
                }
            }
        }

        // if sign is not expected,
        //  default is Signed
        if sign.is_none() {
            sign = Some(Sign::Signed);
        }

        // TODO: add err handler
        if userty.is_some() {
            return (userty.unwrap(), sclass);
        }

        let mut ty: Option<Type> = None;
        // e.g. kind is None => 'signed var;' or 'unsigned var;'
        if kind.is_some() {
            match kind.unwrap() {
                PrimitiveType::Void => ty = Some(Type::Void),
                PrimitiveType::Char => ty = Some(Type::Char(sign.clone().unwrap())),
                PrimitiveType::Float => ty = Some(Type::Float),
                PrimitiveType::Double => ty = Some(Type::Double),
                _ => {}
            }
            if ty.is_some() {
                return (ty.unwrap(), sclass);
            }
        }

        match size {
            Size::Short => ty = Some(Type::Short(sign.clone().unwrap())),
            Size::Normal => ty = Some(Type::Int(sign.clone().unwrap())),
            Size::Long => ty = Some(Type::Long(sign.clone().unwrap())),
            Size::LLong => ty = Some(Type::LLong(sign.clone().unwrap())),
        }

        assert!(ty.is_some(), "ty is None!");
        (ty.unwrap(), sclass)
    }

    fn read_struct_def(&mut self) -> Type {
        self.read_rectype_def(true)
    }
    // rectype is abbreviation of 'record type'
    fn read_rectype_tag(&mut self) -> Option<String> {
        let maybe_tag = self.lexer.get_e();
        if maybe_tag.kind == TokenKind::Identifier {
            Some(maybe_tag.val)
        } else {
            self.lexer.unget(maybe_tag);
            None
        }
    }
    fn read_rectype_def(&mut self, _is_struct: bool) -> Type {
        let tag = || -> String {
            let opt_tag = self.read_rectype_tag();
            if opt_tag.is_some() {
                opt_tag.unwrap()
            } else {
                "".to_string()
            }
        }();
        let fields = self.read_rectype_fields();

        let mut cur_tags = self.tags.back_mut().unwrap();
        match cur_tags.entry(tag) {
            hash_map::Entry::Occupied(o) => o.into_mut().clone(),
            hash_map::Entry::Vacant(v) => {
                let new_rectype = Type::Struct(v.key().to_string(), fields);
                v.insert(new_rectype).clone()
            }
        }
    }
    fn read_rectype_fields(&mut self) -> Vec<AST> {
        if !self.lexer.skip("{") {
            return Vec::new();
        }

        let mut decls: Vec<AST> = Vec::new();
        while !self.lexer.skip("}") {
            self.read_decl(&mut decls);
        }
        decls
    }


    pub fn read_expr(&mut self) -> AST {
        let lhs = self.read_comma();
        lhs
    }
    pub fn read_opt_expr(&mut self) -> AST {
        self.read_expr()
    }
    ////////// operators start from here
    fn read_comma(&mut self) -> AST {
        let mut lhs = self.read_assign();
        while self.lexer.skip(",") {
            let rhs = self.read_assign();
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Comma);
        }
        lhs
    }
    fn read_assign(&mut self) -> AST {
        let mut lhs = self.read_logor();
        if self.lexer.skip("?") {
            return self.read_ternary(lhs);
        }
        while self.lexer.skip("=") {
            let rhs = self.read_assign();
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Assign);
        }
        lhs
    }
    fn read_ternary(&mut self, cond: AST) -> AST {
        let then_expr = self.read_expr();
        self.lexer.expect_skip(":");
        let else_expr = self.read_assign();
        AST::TernaryOp(Rc::new(cond), Rc::new(then_expr), Rc::new(else_expr))
    }
    fn read_logor(&mut self) -> AST {
        let mut lhs = self.read_logand();
        while self.lexer.skip("||") {
            let rhs = self.read_logand();
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LOr);
        }
        lhs
    }
    fn read_logand(&mut self) -> AST {
        let mut lhs = self.read_or();
        while self.lexer.skip("&&") {
            let rhs = self.read_or();
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LAnd);
        }
        lhs
    }
    fn read_or(&mut self) -> AST {
        let mut lhs = self.read_xor();
        while self.lexer.skip("|") {
            let rhs = self.read_xor();
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Or);
        }
        lhs
    }
    fn read_xor(&mut self) -> AST {
        let mut lhs = self.read_and();
        while self.lexer.skip("^") {
            let rhs = self.read_and();
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Xor);
        }
        lhs
    }
    fn read_and(&mut self) -> AST {
        let mut lhs = self.read_eq_ne();
        while self.lexer.skip("&") {
            let rhs = self.read_eq_ne();
            lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::And);
        }
        lhs
    }
    fn read_eq_ne(&mut self) -> AST {
        let mut lhs = self.read_relation();
        loop {
            if self.lexer.skip("==") {
                let rhs = self.read_relation();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Eq);
            } else if self.lexer.skip("!=") {
                let rhs = self.read_relation();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ne);
            } else {
                break;
            }
        }
        lhs
    }
    fn read_relation(&mut self) -> AST {
        let mut lhs = self.read_shl_shr();
        loop {
            if self.lexer.skip("<") {
                let rhs = self.read_shl_shr();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Lt);
            } else if self.lexer.skip("<=") {
                let rhs = self.read_shl_shr();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Le);
            } else if self.lexer.skip(">") {
                let rhs = self.read_shl_shr();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Gt);
            } else if self.lexer.skip(">=") {
                let rhs = self.read_shl_shr();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ge);
            } else {
                break;
            }
        }
        lhs
    }
    fn read_shl_shr(&mut self) -> AST {
        let mut lhs = self.read_add_sub();
        loop {
            if self.lexer.skip("<<") {
                let rhs = self.read_add_sub();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shl);
            } else if self.lexer.skip(">>") {
                let rhs = self.read_add_sub();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shr);
            } else {
                break;
            }
        }
        lhs
    }
    fn read_add_sub(&mut self) -> AST {
        let mut lhs = self.read_mul_div_rem();
        loop {
            if self.lexer.skip("+") {
                let rhs = self.read_mul_div_rem();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Add);
            } else if self.lexer.skip("-") {
                let rhs = self.read_mul_div_rem();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Sub);
            } else {
                break;
            }
        }
        lhs
    }
    fn read_mul_div_rem(&mut self) -> AST {
        let mut lhs = self.read_cast();
        loop {
            if self.lexer.skip("*") {
                let rhs = self.read_cast();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Mul);
            } else if self.lexer.skip("/") {
                let rhs = self.read_cast();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Div);
            } else if self.lexer.skip("%") {
                let rhs = self.read_cast();
                lhs = AST::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Rem);
            } else {
                break;
            }
        }
        lhs
    }
    fn read_cast(&mut self) -> AST {
        self.read_unary()
    }
    fn read_unary(&mut self) -> AST {
        let tok = self.lexer
            .get()
            .or_else(|| error::error_exit(self.lexer.cur_line, "expected unary op"))
            .unwrap();
        if tok.kind == TokenKind::Symbol {
            match tok.val.as_str() { 
                "!" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::LNot),
                "~" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::BNot),
                "+" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Plus),
                "-" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Minus),
                "++" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Inc),
                "--" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Dec),
                "*" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Deref),
                "&" => return AST::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Addr),
                _ => {}
            }
        }
        self.lexer.unget(tok);
        self.read_postfix()
    }
    fn read_postfix(&mut self) -> AST {
        let mut ast = self.read_primary();
        loop {
            if self.lexer.skip("(") {
                ast = self.read_func_call(ast);
                continue;
            }
            if self.lexer.skip("[") {
                ast = self.read_index(ast);
                continue;
            }
            if self.lexer.skip(".") {
                ast = self.read_field(ast);
            }
            if self.lexer.skip("->") {
                ast = self.read_field(AST::UnaryOp(Rc::new(ast), node::CUnaryOps::Deref));
            }
            // TODO: impelment inc and dec
            break;
        }
        ast
    }
    fn read_func_call(&mut self, f: AST) -> AST {
        let mut args: Vec<AST> = Vec::new();
        if !self.lexer.skip(")") {
            loop {
                let arg = self.read_assign();
                args.push(arg);

                if self.lexer.skip(")") {
                    break;
                }
                self.lexer.expect_skip(",");
            }
        }
        AST::FuncCall(Rc::new(f), args)
    }
    fn read_index(&mut self, ast: AST) -> AST {
        let idx = self.read_expr();
        self.lexer.expect_skip("]");
        AST::UnaryOp(Rc::new(AST::BinaryOp(Rc::new(ast), Rc::new(idx), node::CBinOps::AddrAdd)),
                     node::CUnaryOps::Deref)
    }

    fn read_field(&mut self, ast: AST) -> AST {
        let field = self.lexer.get_e();
        if field.kind != TokenKind::Identifier {
            error::error_exit(self.lexer.cur_line, "expected field name");
        }

        let field_name = field.val;
        AST::StructRef(Rc::new(ast), field_name)
    }

    fn read_const_array(&mut self) -> AST {
        let mut elems = Vec::new();
        loop {
            elems.push(self.read_assign());
            if self.lexer.skip("}") {
                break;
            }
            self.lexer.expect_skip(",");
        }
        node::AST::ConstArray(elems)
    }
    fn read_primary(&mut self) -> AST {
        let tok = self.lexer
            .get()
            .or_else(|| error::error_exit(self.lexer.cur_line, "expected primary"))
            .unwrap();
        match tok.kind {
            TokenKind::IntNumber => {
                let num_literal = tok.val;
                if num_literal.len() > 2 && num_literal.chars().nth(1).unwrap() == 'x' {
                    AST::Int(self.read_hex_num(&num_literal[2..]))
                } else {
                    AST::Int(self.read_dec_num(num_literal.as_str()))
                }
            }
            TokenKind::FloatNumber => {
                let num_literal = &tok.val;
                let f: f64 = num_literal.parse().unwrap();
                AST::Float(f)
            }
            TokenKind::Identifier => AST::Variable(tok.val),
            TokenKind::String => AST::String(tok.val),
            TokenKind::Char => {
                let ch = tok.val.bytes().nth(0);
                AST::Char(if ch.is_some() { ch.unwrap() } else { 0 } as i32)
            }
            TokenKind::Symbol => {
                match tok.val.as_str() {
                    "(" => {
                        let expr = self.read_expr();
                        self.lexer.skip(")");
                        expr
                    }
                    "{" => self.read_const_array(),
                    _ => {
                        self.lexer.unget(tok);
                        AST::Compound(Vec::new())
                    }
                }
            }
            // TokenKind::Newline => None,
            _ => {
                error::error_exit(self.lexer.cur_line,
                                  format!("read_primary unknown token {:?}", tok.kind).as_str())
            }
        }
    }
    fn read_dec_num(&mut self, num_literal: &str) -> i32 {
        let mut n = 0;
        for c in num_literal.chars() {
            match c {
                '0'...'9' => n = n * 10 + c.to_digit(10).unwrap() as i32, 
                _ => {} // TODO: suffix
            }
        }
        n
    }
    fn read_hex_num(&mut self, num_literal: &str) -> i32 {
        let mut n = 0;
        for c in num_literal.chars() {
            match c {
                '0'...'9' | 'A'...'F' | 'a'...'f' => {
                    n = n * 16 + c.to_digit(16).unwrap() as i32;
                }
                _ => {} // TODO: suffix
            }
        }
        n
    }
    ////////// operators end here
}
