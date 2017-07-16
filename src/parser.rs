use lexer::{Lexer, Token, TokenKind, Keyword, Symbol};
use node::{AST, ASTKind};
use node;
use error;
use types::{Type, StorageClass, Sign};

use std::{str, u32};
use std::rc::Rc;
use std::collections::{HashMap, VecDeque, hash_map};

extern crate llvm_sys as llvm;

extern crate rand;
use self::rand::Rng;

pub struct Parser<'a> {
    lexer: &'a mut Lexer,
    env: VecDeque<HashMap<String, AST>>,
    tags: VecDeque<HashMap<String, Type>>,
}

fn retrieve_from_load(ast: &AST) -> AST {
    match ast.kind {
        ASTKind::Load(ref var) => (**var).clone(),
        _ => (*ast).clone(),
    }
}

impl<'a> Parser<'a> {
    pub fn new(lexer: &'a mut Lexer) -> Parser<'a> {
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
        let mut lexer = Lexer::new(filename.to_string());
        // TODO: for debugging
        // loop {
        //     let tok = lexer.get();
        //     match tok {
        //         Some(t) => {
        //             println!("t:{}{:?} {}", if t.space { " " } else { "" }, t.kind, t.val);
        //         }
        //         None => break,
        //     }
        // }
        //
        // // Debug: (parsing again is big cost?)
        // lexer = Lexer::new(filename.to_string(), s.as_str());
        Parser::new(&mut lexer).run(&mut nodes);
        nodes
    }
    pub fn run(&mut self, node: &mut Vec<AST>) {
        while self.lexer.peek().is_some() {
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
        let (functy, name, param_names) = self.read_declarator(&retty);
        println!("functy: {:?}", functy);

        self.lexer.expect_skip_symbol(Symbol::OpeningBrace);
        let body = self.read_func_body(&functy);
        self.env.pop_back();
        self.tags.pop_back();
        AST::new(ASTKind::FuncDef(functy,
                                  if param_names.is_none() {
                                      Vec::new()
                                  } else {
                                      param_names.unwrap()
                                  },
                                  name,
                                  Rc::new(body)),
                 0)
    }
    fn read_func_body(&mut self, _functy: &Type) -> AST {
        self.read_compound_stmt()
    }
    fn read_compound_stmt(&mut self) -> AST {
        let mut stmts: Vec<AST> = Vec::new();
        loop {
            if self.lexer.skip_symbol(Symbol::ClosingBrace) {
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
        AST::new(ASTKind::Block(stmts), 0)
    }
    fn read_stmt(&mut self) -> AST {
        let tok = self.lexer.get_e();
        if let &TokenKind::Keyword(ref keyw) = &tok.kind {
            match *keyw {
                Keyword::If => return self.read_if_stmt(),
                Keyword::For => return self.read_for_stmt(),
                Keyword::While => return self.read_while_stmt(),
                Keyword::Return => return self.read_return_stmt(),
                _ => {}
            }
        } else if let &TokenKind::Symbol(Symbol::OpeningBrace) = &tok.kind {
            return self.read_compound_stmt();
        }
        self.lexer.unget(tok);
        let expr = self.read_expr();
        self.lexer.expect_skip_symbol(Symbol::Semicolon);
        expr
    }
    fn read_if_stmt(&mut self) -> AST {
        self.lexer.expect_skip_symbol(Symbol::OpeningParen);
        let cond = self.read_expr();
        self.lexer.expect_skip_symbol(Symbol::ClosingParen);
        let then_stmt = Rc::new(self.read_stmt());
        let else_stmt = if self.lexer.skip_keyword(Keyword::Else) {
            Rc::new(self.read_stmt())
        } else {
            Rc::new(AST::new(ASTKind::Block(Vec::new()), 0))
        };
        AST::new(ASTKind::If(Rc::new(cond), then_stmt, else_stmt), 0)
    }
    fn read_for_stmt(&mut self) -> AST {
        self.lexer.expect_skip_symbol(Symbol::OpeningParen);
        let init = self.read_opt_decl_or_stmt();
        // TODO: make read_expr returns Option<AST>.
        //       when cur tok is ';', returns None.
        let cond = self.read_opt_expr();
        self.lexer.expect_skip_symbol(Symbol::Semicolon);
        let step = self.read_opt_expr();
        self.lexer.expect_skip_symbol(Symbol::ClosingParen);
        let body = self.read_stmt();
        AST::new(ASTKind::For(Rc::new(init), Rc::new(cond), Rc::new(step), Rc::new(body)),
                 0)
    }
    fn read_while_stmt(&mut self) -> AST {
        self.lexer.expect_skip_symbol(Symbol::OpeningParen);
        let cond = self.read_expr();
        self.lexer.expect_skip_symbol(Symbol::ClosingParen);
        let body = self.read_stmt();
        AST::new(ASTKind::While(Rc::new(cond), Rc::new(body)), 0)
    }
    fn read_return_stmt(&mut self) -> AST {
        let line = *self.lexer.cur_line.back().unwrap();
        if self.lexer.skip_symbol(Symbol::Semicolon) {
            AST::new(ASTKind::Return(None), line)
        } else {
            let retval = Some(Rc::new(self.read_expr()));
            self.lexer.expect_skip_symbol(Symbol::Semicolon);
            AST::new(ASTKind::Return(retval), line)
        }
    }
    fn is_function_def(&mut self) -> bool {
        let mut buf = Vec::new();
        let mut is_funcdef = false;

        loop {
            let mut tok = self.lexer.get().unwrap();
            buf.push(tok.clone());

            if tok.kind == TokenKind::Symbol(Symbol::Semicolon) {
                break;
            }

            if self.is_type(&tok) {
                continue;
            }

            if tok.kind == TokenKind::Symbol(Symbol::OpeningParen) {
                self.skip_brackets(&mut buf);
                continue;
            }

            if tok.kind != TokenKind::Identifier {
                continue;
            }

            if self.lexer.peek().unwrap().kind != TokenKind::Symbol(Symbol::OpeningParen) {
                continue;
            }

            buf.push(self.lexer.get().unwrap());
            self.skip_brackets(&mut buf);

            tok = self.lexer.peek().unwrap();
            is_funcdef = tok.kind == TokenKind::Symbol(Symbol::OpeningBrace);
            break;
        }

        self.lexer.unget_all(buf);
        is_funcdef
    }
    fn skip_brackets(&mut self, buf: &mut Vec<Token>) {
        loop {
            let tok = self.lexer.get().unwrap();
            buf.push(tok.clone());

            match tok.kind {
                TokenKind::Symbol(Symbol::OpeningParen) => self.skip_brackets(buf),
                TokenKind::Symbol(Symbol::ClosingParen) => break,
                _ => {}
            }
        }
    }

    fn get_typedef(&mut self, name: &str) -> Option<Type> {
        self.env
            .back()
            .unwrap()
            .get(name)
            .and_then(|ast| {
                Some(match ast.kind {
                         ASTKind::Typedef(ref from, ref _to) => (*from).clone(),
                         _ => {
                             error::error_exit(*self.lexer.cur_line.back().unwrap(),
                                               format!("not found type '{}'", name).as_str())
                         }
                     })
            })
    }
    fn is_type(&mut self, token: &Token) -> bool {
        if let TokenKind::Keyword(ref keyw) = token.kind {
            match *keyw {
                Keyword::Typedef | Keyword::Extern | Keyword::Static | Keyword::Auto |
                Keyword::Register | Keyword::Const | Keyword::Volatile | Keyword::Void |
                Keyword::Signed | Keyword::Unsigned | Keyword::Char | Keyword::Int |
                Keyword::Short | Keyword::Long | Keyword::Float | Keyword::Double |
                Keyword::Struct | Keyword::Enum | Keyword::Union | Keyword::Noreturn |
                Keyword::Inline | Keyword::Restrict => true,
                _ => false,
            }
        } else if token.kind == TokenKind::Identifier {
            self.env
                .back_mut()
                .unwrap()
                .contains_key(token.val.as_str())
        } else {
            false
        }
    }
    fn read_decl_init(&mut self, ty: &mut Type) -> AST {
        // TODO: implement for like 'int a[] = {...}, char *s="str";'
        if self.lexer.skip_symbol(Symbol::OpeningBrace) {
            self.read_initializer_list(ty)
        } else if let TokenKind::String(_) = self.lexer.peek_e().kind {
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
            loop {
                if self.lexer.skip_symbol(Symbol::ClosingBrace) {
                    break;
                }
                let elem = self.read_assign();
                elems.push(elem);
                self.lexer.skip_symbol(Symbol::Comma);
            }
            if is_flexible {
                *len = elems.len() as i32;
            }
            AST::new(ASTKind::ConstArray(elems),
                     *self.lexer.cur_line.back().unwrap())
        } else {
            error::error_exit(*self.lexer.cur_line.back().unwrap(), "impossible");
        }
        // self.read_assign()
    }
    fn skip_type_qualifiers(&mut self) {
        while self.lexer.skip_keyword(Keyword::Const) ||
              self.lexer.skip_keyword(Keyword::Volatile) ||
              self.lexer.skip_keyword(Keyword::Restrict) {}
    }
    fn read_decl(&mut self, ast: &mut Vec<AST>) {
        let (basety, sclass_w) = self.read_type_spec();
        let (sclass, is_typedef) = if let Some(sclass) = sclass_w {
            (sclass.clone(), sclass == StorageClass::Typedef)
        } else {
            (StorageClass::Auto, false)
        };

        if self.lexer.skip_symbol(Symbol::Semicolon) {
            return;
        }

        loop {
            let (mut ty, name, _) = self.read_declarator(&basety);

            if is_typedef {
                let typedef = AST::new(ASTKind::Typedef(ty, name.to_string()),
                                       *self.lexer.cur_line.back().unwrap());
                self.env.back_mut().unwrap().insert(name, typedef);
                return;
            }

            let init = if self.lexer.skip_symbol(Symbol::Assign) {
                Some(Rc::new(self.read_decl_init(&mut ty)))
            } else {
                None
            };
            ast.push(AST::new(ASTKind::VariableDecl(ty, name, sclass.clone(), init),
                              *self.lexer.cur_line.back().unwrap()));

            if self.lexer.skip_symbol(Symbol::Semicolon) {
                return;
            }
            self.lexer.expect_skip_symbol(Symbol::Comma);
        }
    }
    fn read_opt_decl_or_stmt(&mut self) -> AST {
        if self.lexer.skip_symbol(Symbol::Semicolon) {
            return AST::new(ASTKind::Compound(Vec::new()),
                            *self.lexer.cur_line.back().unwrap());
        }

        let peek_tok = self.lexer.peek_e();
        if self.is_type(&peek_tok) {
            // variable declaration
            let mut stmts = Vec::new();
            let line = *self.lexer.cur_line.back().unwrap();
            self.read_decl(&mut stmts);
            AST::new(ASTKind::Compound(stmts), line)
        } else {
            self.read_stmt()
        }
    }
    // returns (declarator type, name, params{for function})
    fn read_declarator(&mut self, basety: &Type) -> (Type, String, Option<Vec<String>>) {
        if self.lexer.skip_symbol(Symbol::OpeningParen) {
            let peek_tok = self.lexer.peek_e();
            if self.is_type(&peek_tok) {
                let (ty, params) = self.read_declarator_func(basety);
                return (ty, "".to_string(), params);
            }

            // TODO: HUH? MAKES NO SENSE!!
            let mut buf: Vec<Token> = Vec::new();
            while !self.lexer.skip_symbol(Symbol::ClosingParen) {
                buf.push(self.lexer.get().unwrap());
            }
            let t = self.read_declarator_tail(basety);
            self.lexer.unget_all(buf);
            return self.read_declarator(&t.0);
        }

        if self.lexer.skip_symbol(Symbol::Asterisk) {
            self.skip_type_qualifiers();
            return self.read_declarator(&Type::Ptr(Rc::new(basety.clone())));
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
    fn read_declarator_tail(&mut self, basety: &Type) -> (Type, Option<Vec<String>>) {
        if self.lexer.skip_symbol(Symbol::OpeningBoxBracket) {
            return (self.read_declarator_array(basety), None);
        }
        if self.lexer.skip_symbol(Symbol::OpeningParen) {
            return self.read_declarator_func(basety);
        }
        (basety.clone(), None)
    }

    fn read_declarator_array(&mut self, basety: &Type) -> Type {
        let len: i32;
        if self.lexer.skip_symbol(Symbol::ClosingBoxBracket) {
            len = -1;
        } else {
            len = self.read_expr().eval_constexpr() as i32;
            self.lexer.expect_skip_symbol(Symbol::ClosingBoxBracket);
        }
        let ty = self.read_declarator_tail(basety).0;
        Type::Array(Rc::new(ty), len)
    }
    fn read_declarator_func(&mut self, retty: &Type) -> (Type, Option<Vec<String>>) {
        if self.lexer.peek_keyword_token_is(Keyword::Void) &&
           self.lexer.next_symbol_token_is(Symbol::ClosingParen) {
            self.lexer.expect_skip_keyword(Keyword::Void);
            self.lexer.expect_skip_symbol(Symbol::ClosingParen);
            return (Type::Func(Rc::new(retty.clone()), Vec::new(), false), None);
        }
        if self.lexer.skip_symbol(Symbol::ClosingParen) {
            return (Type::Func(Rc::new(retty.clone()), Vec::new(), false), None);
        }

        let (paramtypes, paramnames, vararg) = self.read_declarator_params();
        (Type::Func(Rc::new(retty.clone()), paramtypes.clone(), vararg), Some(paramnames))
    }
    // returns (param types, param names, vararg?)
    fn read_declarator_params(&mut self) -> (Vec<Type>, Vec<String>, bool) {
        let mut paramtypes: Vec<Type> = Vec::new();
        let mut paramnames: Vec<String> = Vec::new();
        loop {
            if self.lexer.skip_symbol(Symbol::Vararg) {
                if paramtypes.len() == 0 {
                    error::error_exit(*self.lexer.cur_line.back().unwrap(),
                                      "at least one param is required before '...'");
                }
                self.lexer.expect_skip_symbol(Symbol::ClosingParen);
                return (paramtypes, paramnames, true);
            }

            let (ty, name) = self.read_func_param();
            paramtypes.push(ty);
            paramnames.push(name);
            if self.lexer.skip_symbol(Symbol::ClosingParen) {
                return (paramtypes, paramnames, false);
            }
            self.lexer.expect_skip_symbol(Symbol::Comma);
        }
    }
    fn read_func_param(&mut self) -> (Type, String) {
        let basety = self.read_type_spec().0;
        let (ty, name, _) = self.read_declarator(&basety);
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
            error::error_exit(*lexer.cur_line.back().unwrap(), "type mismatch");
        };
        let err_sign = |lexer: &Lexer, sign: Option<Sign>| if sign.is_some() {
            error::error_exit(*lexer.cur_line.back().unwrap(), "type mismatch");
        };

        // let mut env = ENV_MAP.lock().unwrap();

        loop {
            let tok = self.lexer
                .get()
                .or_else(|| {
                             error::error_exit(*self.lexer.cur_line.back().unwrap(),
                                               "expect types but reach EOF")
                         })
                .unwrap();

            if kind.is_none() && tok.kind == TokenKind::Identifier {
                let maybe_userty_name = tok.val.as_str();
                let maybe_userty = self.get_typedef(maybe_userty_name);
                if maybe_userty.is_some() {
                    return (maybe_userty.unwrap(), sclass);
                }
            }

            if let TokenKind::Keyword(keyw) = tok.kind {
                match &keyw {
                    &Keyword::Typedef => sclass = Some(StorageClass::Typedef),
                    &Keyword::Extern => sclass = Some(StorageClass::Extern),
                    &Keyword::Static => sclass = Some(StorageClass::Static),
                    &Keyword::Auto => sclass = Some(StorageClass::Auto),
                    &Keyword::Register => sclass = Some(StorageClass::Register),
                    &Keyword::Const |
                    &Keyword::Volatile |
                    &Keyword::Inline |
                    &Keyword::Restrict |
                    &Keyword::Noreturn => {}
                    &Keyword::Void => {
                        err_kind(&self.lexer, kind);
                        kind = Some(PrimitiveType::Void);
                    }
                    &Keyword::Char => {
                        err_kind(&self.lexer, kind);
                        kind = Some(PrimitiveType::Char);
                    }
                    &Keyword::Int => {
                        err_kind(&self.lexer, kind);
                        kind = Some(PrimitiveType::Int);
                    }
                    &Keyword::Float => {
                        err_kind(&self.lexer, kind);
                        kind = Some(PrimitiveType::Float);
                    }
                    &Keyword::Double => {
                        err_kind(&self.lexer, kind);
                        kind = Some(PrimitiveType::Double);
                    }
                    &Keyword::Signed => {
                        err_sign(&self.lexer, sign);
                        sign = Some(Sign::Signed);
                    }
                    &Keyword::Unsigned => {
                        err_sign(&self.lexer, sign);
                        sign = Some(Sign::Unsigned);
                    }
                    &Keyword::Short => size = Size::Short,
                    &Keyword::Long => {
                        if size == Size::Normal {
                            size = Size::Long;
                        } else if size == Size::Long {
                            size = Size::LLong;
                        }
                    }
                    &Keyword::Struct => userty = Some(self.read_struct_def()),
                    &Keyword::Union => userty = Some(self.read_union_def()),
                    &Keyword::Enum => userty = Some(self.read_enum_def()),
                    _ => {}
                }
            } else {
                self.lexer.unget(tok);
                break;
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

        if kind.is_some() {
            match kind.unwrap() {
                PrimitiveType::Void => return (Type::Void, sclass),
                PrimitiveType::Char => return (Type::Char(sign.clone().unwrap()), sclass),
                PrimitiveType::Float => return (Type::Float, sclass),
                PrimitiveType::Double => return (Type::Double, sclass),
                _ => {}
            }
        }

        let ty = match size {
            Size::Short => Type::Short(sign.clone().unwrap()),
            Size::Normal => Type::Int(sign.clone().unwrap()),
            Size::Long => Type::Long(sign.clone().unwrap()),
            Size::LLong => Type::LLong(sign.clone().unwrap()),
        };

        (ty, sclass)
    }

    fn read_struct_def(&mut self) -> Type {
        self.read_rectype_def(true)
    }
    fn read_union_def(&mut self) -> Type {
        self.read_rectype_def(false)
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
    fn read_rectype_def(&mut self, is_struct: bool) -> Type {
        let tag = || -> String {
            let opt_tag = self.read_rectype_tag();
            if opt_tag.is_some() {
                opt_tag.unwrap()
            } else {
                // if the rectype(struct|union) has no name(e.g. typedef struct { int a; } A),
                // generate a random name
                rand::thread_rng().gen_ascii_chars().take(8).collect()
            }
        }();
        let fields = self.read_rectype_fields();

        let mut cur_tags = self.tags.back_mut().unwrap();
        if fields.is_empty() {
            match cur_tags.entry(tag) {
                hash_map::Entry::Occupied(o) => o.into_mut().clone(),
                hash_map::Entry::Vacant(v) => {
                    let new_struct = if is_struct {
                        Type::Struct(v.key().to_string(), Vec::new())
                    } else {
                        Type::Union(v.key().to_string(), Vec::new(), 0)
                    };
                    v.insert(new_struct).clone()
                }
            }
        } else {
            let new_rectype = if is_struct {
                Type::Struct(tag.to_string(), fields)
            } else {
                // if union
                let mut max_sz_ty_nth = 0;
                let mut max_sz = 0;
                for (i, field_decl) in (&fields).iter().enumerate() {
                    if let ASTKind::VariableDecl(ref ty, _, _, _) = field_decl.kind {
                        if ty.calc_size() > max_sz {
                            max_sz = ty.calc_size();
                            max_sz_ty_nth = i;
                        }
                    }
                }
                Type::Union(tag.to_string(), fields, max_sz_ty_nth)
            };
            match cur_tags.entry(tag) {
                hash_map::Entry::Occupied(o) => {
                    *o.into_mut() = new_rectype.clone();
                    new_rectype
                }
                hash_map::Entry::Vacant(v) => v.insert(new_rectype).clone(),
            }
        }
    }
    fn read_rectype_fields(&mut self) -> Vec<AST> {
        if !self.lexer.skip_symbol(Symbol::OpeningBrace) {
            return Vec::new();
        }

        let mut decls: Vec<AST> = Vec::new();
        loop {
            let peek = self.lexer.peek_e();
            if !self.is_type(&peek) {
                break;
            }
            let (basety, _) = self.read_type_spec();
            loop {
                let (ty, name, _) = self.read_declarator(&basety.clone());
                if self.lexer.skip_symbol(Symbol::Colon) {
                    // TODO: for now, designated bitwidth ignore
                    self.read_expr();
                }
                decls.push(AST::new(ASTKind::VariableDecl(ty, name, StorageClass::Auto, None),
                                    *self.lexer.cur_line.back().unwrap()));
                if self.lexer.skip_symbol(Symbol::Comma) {
                    continue;
                } else {
                    self.lexer.expect_skip_symbol(Symbol::Semicolon);
                }
                break;
            }
        }
        self.lexer.expect_skip_symbol(Symbol::ClosingBrace);
        decls
    }
    fn read_enum_def(&mut self) -> Type {
        let (tag, exist_tag) = || -> (String, bool) {
            let opt_tag = self.read_rectype_tag();
            if opt_tag.is_some() {
                (opt_tag.unwrap(), true)
            } else {
                ("".to_string(), false)
            }
        }();
        if exist_tag {
            if let Some(maybe_enum) = self.tags.back_mut().unwrap().get(tag.as_str()) {
                match maybe_enum {
                    &Type::Enum => {}
                    _ => error::error_exit(*self.lexer.cur_line.back().unwrap(), "undefined enum"),
                }
            }
        }

        if !self.lexer.skip_symbol(Symbol::OpeningBrace) {
            if !exist_tag ||
               !self.tags
                    .back_mut()
                    .unwrap()
                    .contains_key(tag.as_str()) {
                error::error_exit(*self.lexer.cur_line.back().unwrap(), "defined enum");
            }
            return Type::Int(Sign::Signed);
        }

        if exist_tag {
            self.tags.back_mut().unwrap().insert(tag, Type::Enum);
        }

        let mut val = 0;
        loop {
            if self.lexer.skip_symbol(Symbol::ClosingBrace) {
                break;
            }
            let name = self.lexer.get_e().val;
            if self.lexer.skip_symbol(Symbol::Assign) {
                val = self.read_assign().eval_constexpr();
            }
            let constval = AST::new(ASTKind::Int(val), *self.lexer.cur_line.back().unwrap());
            val += 1;
            self.env.back_mut().unwrap().insert(name, constval);
            if self.lexer.skip_symbol(Symbol::Comma) {
                continue;
            }
            if self.lexer.skip_symbol(Symbol::OpeningBrace) {
                break;
            }
        }

        Type::Int(Sign::Signed)
    }


    pub fn read_expr(&mut self) -> AST {
        self.read_comma()
    }
    pub fn read_opt_expr(&mut self) -> AST {
        self.read_expr()
    }
    fn read_comma(&mut self) -> AST {
        let mut lhs = self.read_assign();
        while self.lexer.skip_symbol(Symbol::Comma) {
            let rhs = self.read_assign();
            lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Comma),
                           *self.lexer.cur_line.back().unwrap())
        }
        lhs
    }
    fn read_assign(&mut self) -> AST {
        let mut lhs = self.read_logor();
        if self.lexer.skip_symbol(Symbol::Question) {
            return self.read_ternary(lhs);
        }
        let assign = |lhs, rhs, line| -> AST {
            AST::new(ASTKind::BinaryOp(Rc::new(retrieve_from_load(&lhs)),
                                       Rc::new(rhs),
                                       node::CBinOps::Assign),
                     line)
        };
        loop {
            let tok = self.lexer.get_e();
            match tok.kind {
                TokenKind::Symbol(Symbol::Assign) => {
                    lhs = assign(lhs,
                                 self.read_assign(),
                                 *self.lexer.cur_line.back().unwrap());
                }
                TokenKind::Symbol(Symbol::AssignAdd) => {
                    lhs = assign(lhs.clone(),
                                 AST::new(ASTKind::BinaryOp(Rc::new(lhs),
                                                            Rc::new(self.read_assign()),
                                                            node::CBinOps::Add),
                                          *self.lexer.cur_line.back().unwrap()),
                                 *self.lexer.cur_line.back().unwrap());
                }
                TokenKind::Symbol(Symbol::AssignSub) => {
                    lhs = assign(lhs.clone(),
                                 AST::new(ASTKind::BinaryOp(Rc::new(lhs),
                                                            Rc::new(self.read_assign()),
                                                            node::CBinOps::Sub),
                                          *self.lexer.cur_line.back().unwrap()),
                                 *self.lexer.cur_line.back().unwrap());
                }
                TokenKind::Symbol(Symbol::AssignMul) => {
                    lhs = assign(lhs.clone(),
                                 AST::new(ASTKind::BinaryOp(Rc::new(lhs),
                                                            Rc::new(self.read_assign()),
                                                            node::CBinOps::Mul),
                                          *self.lexer.cur_line.back().unwrap()),
                                 *self.lexer.cur_line.back().unwrap());
                }
                TokenKind::Symbol(Symbol::AssignDiv) => {
                    lhs = assign(lhs.clone(),
                                 AST::new(ASTKind::BinaryOp(Rc::new(lhs),
                                                            Rc::new(self.read_assign()),
                                                            node::CBinOps::Div),
                                          *self.lexer.cur_line.back().unwrap()),
                                 *self.lexer.cur_line.back().unwrap());
                }
                TokenKind::Symbol(Symbol::AssignMod) => {
                    lhs = assign(lhs.clone(),
                                 AST::new(ASTKind::BinaryOp(Rc::new(lhs),
                                                            Rc::new(self.read_assign()),
                                                            node::CBinOps::Rem),
                                          *self.lexer.cur_line.back().unwrap()),
                                 *self.lexer.cur_line.back().unwrap());
                }
                // TODO: implement more op
                _ => {
                    self.lexer.unget(tok);
                    break;
                }
            }
        }
        lhs
    }
    fn read_ternary(&mut self, cond: AST) -> AST {
        let then_expr = self.read_expr();
        self.lexer.expect_skip_symbol(Symbol::Colon);
        let else_expr = self.read_assign();
        AST::new(ASTKind::TernaryOp(Rc::new(cond), Rc::new(then_expr), Rc::new(else_expr)),
                 *self.lexer.cur_line.back().unwrap())
    }
    fn read_logor(&mut self) -> AST {
        let mut lhs = self.read_logand();
        while self.lexer.skip_symbol(Symbol::LOr) {
            let rhs = self.read_logand();
            lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LOr),
                           *self.lexer.cur_line.back().unwrap());
        }
        lhs
    }
    fn read_logand(&mut self) -> AST {
        let mut lhs = self.read_or();
        while self.lexer.skip_symbol(Symbol::LAnd) {
            let rhs = self.read_or();
            lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::LAnd),
                           *self.lexer.cur_line.back().unwrap());
        }
        lhs
    }
    fn read_or(&mut self) -> AST {
        let mut lhs = self.read_xor();
        while self.lexer.skip_symbol(Symbol::Or) {
            let rhs = self.read_xor();
            lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Or),
                           *self.lexer.cur_line.back().unwrap());
        }
        lhs
    }
    fn read_xor(&mut self) -> AST {
        let mut lhs = self.read_and();
        while self.lexer.skip_symbol(Symbol::Xor) {
            let rhs = self.read_and();
            lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Xor),
                           *self.lexer.cur_line.back().unwrap());
        }
        lhs
    }
    fn read_and(&mut self) -> AST {
        let mut lhs = self.read_eq_ne();
        while self.lexer.skip_symbol(Symbol::Ampersand) {
            let rhs = self.read_eq_ne();
            lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::And),
                           *self.lexer.cur_line.back().unwrap());
        }
        lhs
    }
    fn read_eq_ne(&mut self) -> AST {
        let mut lhs = self.read_relation();
        loop {
            if self.lexer.skip_symbol(Symbol::Eq) {
                let rhs = self.read_relation();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Eq),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Ne) {
                let rhs = self.read_relation();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ne),
                               *self.lexer.cur_line.back().unwrap());
            } else {
                break;
            }
        }
        lhs
    }
    fn read_relation(&mut self) -> AST {
        let mut lhs = self.read_shl_shr();
        loop {
            if self.lexer.skip_symbol(Symbol::Lt) {
                let rhs = self.read_shl_shr();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Lt),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Le) {
                let rhs = self.read_shl_shr();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Le),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Gt) {
                let rhs = self.read_shl_shr();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Gt),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Ge) {
                let rhs = self.read_shl_shr();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Ge),
                               *self.lexer.cur_line.back().unwrap());
            } else {
                break;
            }
        }
        lhs
    }
    fn read_shl_shr(&mut self) -> AST {
        let mut lhs = self.read_add_sub();
        loop {
            if self.lexer.skip_symbol(Symbol::Shl) {
                let rhs = self.read_add_sub();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shl),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Shr) {
                let rhs = self.read_add_sub();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Shr),
                               *self.lexer.cur_line.back().unwrap());
            } else {
                break;
            }
        }
        lhs
    }
    fn read_add_sub(&mut self) -> AST {
        let mut lhs = self.read_mul_div_rem();
        loop {
            if self.lexer.skip_symbol(Symbol::Add) {
                let rhs = self.read_mul_div_rem();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Add),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Sub) {
                let rhs = self.read_mul_div_rem();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Sub),
                               *self.lexer.cur_line.back().unwrap());
            } else {
                break;
            }
        }
        lhs
    }
    fn read_mul_div_rem(&mut self) -> AST {
        let mut lhs = self.read_cast();
        loop {
            if self.lexer.skip_symbol(Symbol::Asterisk) {
                let rhs = self.read_cast();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Mul),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Div) {
                let rhs = self.read_cast();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Div),
                               *self.lexer.cur_line.back().unwrap());
            } else if self.lexer.skip_symbol(Symbol::Mod) {
                let rhs = self.read_cast();
                lhs = AST::new(ASTKind::BinaryOp(Rc::new(lhs), Rc::new(rhs), node::CBinOps::Rem),
                               *self.lexer.cur_line.back().unwrap());
            } else {
                break;
            }
        }
        lhs
    }
    fn read_cast(&mut self) -> AST {
        let tok = self.lexer.get_e();
        let peek = self.lexer.peek_e();
        if tok.kind == TokenKind::Symbol(Symbol::OpeningParen) && self.is_type(&peek) {
            let basety = self.read_type_spec().0;
            let ty = self.read_declarator(&basety).0;
            self.lexer.expect_skip_symbol(Symbol::ClosingParen);
            return AST::new(ASTKind::TypeCast(Rc::new(self.read_cast()), ty),
                            *self.lexer.cur_line.back().unwrap());
        } else {
            self.lexer.unget(tok);
        }
        self.read_unary()
    }
    fn read_unary(&mut self) -> AST {
        let tok = self.lexer
            .get()
            .or_else(|| {
                         error::error_exit(*self.lexer.cur_line.back().unwrap(),
                                           "expected unary op")
                     })
            .unwrap();
        match tok.kind { 
            TokenKind::Symbol(Symbol::Not) => {
                return AST::new(ASTKind::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::LNot),
                                *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Symbol(Symbol::BitwiseNot) => {
                return AST::new(ASTKind::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::BNot),
                                *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Symbol(Symbol::Add) => {
                return AST::new(ASTKind::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Plus),
                                *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Symbol(Symbol::Sub) => {
                return AST::new(ASTKind::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Minus),
                                *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Symbol(Symbol::Inc) => {
                let line = *self.lexer.cur_line.back().unwrap();
                let var = self.read_cast();
                return AST::new(ASTKind::BinaryOp(Rc::new(retrieve_from_load(&var)),
                                                  Rc::new(
                                                      AST::new(
                                                          ASTKind::BinaryOp(Rc::new(var), Rc::new(AST::new(ASTKind::Int(1), line)), node::CBinOps::Add), line)),
                                                  node::CBinOps::Assign),
                                line);
            }
            TokenKind::Symbol(Symbol::Dec) => {
                let line = *self.lexer.cur_line.back().unwrap();
                let var = self.read_cast();
                return AST::new(ASTKind::BinaryOp(Rc::new(retrieve_from_load(&var)),
                                                  Rc::new(
                                                      AST::new(
                                                          ASTKind::BinaryOp(Rc::new(var), Rc::new(AST::new(ASTKind::Int(1), line)), node::CBinOps::Sub), line)),
                                                  node::CBinOps::Assign),
                                line);
            }
            TokenKind::Symbol(Symbol::Asterisk) => {
                return AST::new(ASTKind::UnaryOp(Rc::new(self.read_cast()), node::CUnaryOps::Deref),
                                *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Symbol(Symbol::Ampersand) => return retrieve_from_load(&self.read_cast()),
            TokenKind::Symbol(Symbol::Sizeof) => {
                // TODO: must fix this sloppy implementation
                self.lexer.expect_skip_symbol(Symbol::OpeningParen);
                let tok = self.lexer.peek_e();
                assert!(self.is_type(&tok));
                let (basety, _) = self.read_type_spec();
                let (ty, _, _) = self.read_declarator(&basety);
                self.lexer.expect_skip_symbol(Symbol::ClosingParen);
                return AST::new(ASTKind::Int(ty.calc_size() as i64),
                                *self.lexer.cur_line.back().unwrap());
            }
            _ => {}
        }
        self.lexer.unget(tok);
        self.read_postfix()
    }
    fn read_postfix(&mut self) -> AST {
        let mut ast = self.read_primary();
        loop {
            if self.lexer.skip_symbol(Symbol::OpeningParen) {
                ast = self.read_func_call(retrieve_from_load(&ast));
                continue;
            }
            if self.lexer.skip_symbol(Symbol::OpeningBoxBracket) {
                ast = AST::new(ASTKind::Load(Rc::new(self.read_index(ast))),
                               *self.lexer.cur_line.back().unwrap());
                continue;
            }
            if self.lexer.skip_symbol(Symbol::Point) {
                ast = AST::new(ASTKind::Load(Rc::new(self.read_field(retrieve_from_load(&ast)))),
                               *self.lexer.cur_line.back().unwrap());
                continue;
            }
            if self.lexer.skip_symbol(Symbol::Arrow) {
                let line = *self.lexer.cur_line.back().unwrap();
                let field =
                    self.read_field(AST::new(ASTKind::UnaryOp(Rc::new(retrieve_from_load(&ast)),
                                                              node::CUnaryOps::Deref),
                                             line));
                ast = AST::new(ASTKind::Load(Rc::new(field)), line);
                continue;
            }
            if self.lexer.skip_symbol(Symbol::Inc) {
                return AST::new(ASTKind::UnaryOp(Rc::new(ast), node::CUnaryOps::Inc),
                                *self.lexer.cur_line.back().unwrap());
            }
            if self.lexer.skip_symbol(Symbol::Dec) {
                return AST::new(ASTKind::UnaryOp(Rc::new(ast), node::CUnaryOps::Dec),
                                *self.lexer.cur_line.back().unwrap());
            }
            break;
        }
        ast
    }
    fn read_func_call(&mut self, f: AST) -> AST {
        let mut args: Vec<AST> = Vec::new();
        if !self.lexer.skip_symbol(Symbol::ClosingParen) {
            loop {
                let arg = self.read_assign();
                args.push(arg);

                if self.lexer.skip_symbol(Symbol::ClosingParen) {
                    break;
                }
                self.lexer.expect_skip_symbol(Symbol::Comma);
            }
        }
        AST::new(ASTKind::FuncCall(Rc::new(f), args),
                 *self.lexer.cur_line.back().unwrap())
    }
    fn read_index(&mut self, ast: AST) -> AST {
        let idx = self.read_expr();
        self.lexer.expect_skip_symbol(Symbol::ClosingBoxBracket);
        AST::new(ASTKind::BinaryOp(Rc::new(ast), Rc::new(idx), node::CBinOps::Add),
                 *self.lexer.cur_line.back().unwrap())
    }

    fn read_field(&mut self, ast: AST) -> AST {
        let field = self.lexer.get_e();
        if field.kind != TokenKind::Identifier {
            error::error_exit(*self.lexer.cur_line.back().unwrap(), "expected field name");
        }

        let field_name = field.val;
        AST::new(ASTKind::StructRef(Rc::new(ast), field_name),
                 *self.lexer.cur_line.back().unwrap())
    }

    fn read_const_array(&mut self) -> AST {
        let mut elems = Vec::new();
        loop {
            elems.push(self.read_assign());
            if self.lexer.skip_symbol(Symbol::OpeningBrace) {
                break;
            }
            self.lexer.expect_skip_symbol(Symbol::Comma);
        }
        AST::new(ASTKind::ConstArray(elems),
                 *self.lexer.cur_line.back().unwrap())
    }
    fn read_primary(&mut self) -> AST {
        let tok = self.lexer
            .get()
            .or_else(|| {
                         error::error_exit(*self.lexer.cur_line.back().unwrap(), "expected primary")
                     })
            .unwrap();
        match tok.kind.clone() {
            TokenKind::IntNumber(n) => {
                AST::new(ASTKind::Int(n), *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::FloatNumber(f) => {
                AST::new(ASTKind::Float(f), *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Identifier => {
                if let Some(ast) = self.env.back_mut().unwrap().get(tok.val.as_str()) {
                    ast.clone()
                } else {
                    AST::new(ASTKind::Load(Rc::new(AST::new(ASTKind::Variable(tok.val),
                                                            *self.lexer.cur_line.back().unwrap()))),
                             *self.lexer.cur_line.back().unwrap())
                }
            }
            TokenKind::String(s) => {
                AST::new(ASTKind::String(s), *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Char => {
                let ch = tok.val.bytes().nth(0);
                AST::new(ASTKind::Char(if ch.is_some() { ch.unwrap() } else { 0 } as i32),
                         *self.lexer.cur_line.back().unwrap())
            }
            TokenKind::Symbol(sym) => {
                match sym {
                    Symbol::OpeningParen => {
                        let expr = self.read_expr();
                        self.lexer.skip_symbol(Symbol::ClosingParen);
                        expr
                    }
                    Symbol::OpeningBrace => self.read_const_array(),
                    _ => {
                        self.lexer.unget(tok);
                        AST::new(ASTKind::Compound(Vec::new()),
                                 *self.lexer.cur_line.back().unwrap())
                    }
                }
            }
            // TokenKind::Newline => None,
            _ => {
                error::error_exit(*self.lexer.cur_line.back().unwrap(),
                                  format!("read_primary unknown token {:?}", tok.kind).as_str())
            }
        }
    }
}
