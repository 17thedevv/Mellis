use crate::{SemanticContext, ty::{SemanticTypeId, SemanticType, BuiltinType}};
use mellis_ast::{AstArena, Item, Stmt, Expr, Decl};
use mellis_lexer::{BuiltinKind, TokenKind};
use mellis_common::diagnostic::Diagnostic;

pub struct TypeChecker<'a> {
    ctx: &'a mut SemanticContext,
    arena: &'a AstArena,
    source: &'a str,
    is_unsafe_context: bool,
}

impl<'a> TypeChecker<'a> {
    pub fn new(ctx: &'a mut SemanticContext, arena: &'a AstArena, source: &'a str) -> Self {
        Self { ctx, arena, source, is_unsafe_context: false }
    }
    
    pub fn unify(&mut self, expected: SemanticTypeId, actual: SemanticTypeId) -> Result<(), String> {
        let t1 = self.ctx.types.get(expected).clone();
        let t2 = self.ctx.types.get(actual).clone();
        
        if expected == actual {
            return Ok(());
        }
        
        match (t1, t2) {
            (SemanticType::InferenceVar(_), _) => {
                // Simplistic unification: bind var to type (not actually updating context yet)
                Ok(())
            }
            (_, SemanticType::InferenceVar(_)) => {
                Ok(())
            }
            _ => Err(format!("Type mismatch")),
        }
    }

    pub fn typecheck_items(&mut self, items: &[Item]) {
        self.populate_signatures(items);
        for item in items {
            self.typecheck_item(item);
        }
    }

    fn populate_signatures(&mut self, items: &[Item]) {
        // Pass 1: Declare all struct/enum/type types
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Struct { fields, .. } => {
                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();
                        let mut field_tys = Vec::new();
                        for field in fields {
                            field_tys.push(self.lower_type(field.ty));
                        }
                        if let Some(sym_id) = sym_id_opt {
                            let struct_ty = self.ctx.types.intern(SemanticType::Struct(sym_id, field_tys));
                            self.ctx.tables.symbol_types.insert(sym_id, struct_ty);
                        }
                    }
                    Decl::Enum { variants, .. } => {
                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();
                        let mut variant_tys = Vec::new();
                        for variant in variants {
                            // Each variant's type is based on its fields
                            let mut field_tys = Vec::new();
                            for field_id in &variant.fields {
                                if let Decl::Param { ty, .. } = &self.arena.decls[field_id.0 as usize] {
                                    if let Some(t) = ty {
                                        field_tys.push(self.lower_type(*t));
                                    }
                                }
                            }
                            // For enums with no fields on a variant, push a void type
                            if field_tys.is_empty() {
                                variant_tys.push(self.ctx.types.intern(SemanticType::Void));
                            } else if field_tys.len() == 1 {
                                variant_tys.push(field_tys[0]);
                            } else {
                                variant_tys.push(self.ctx.types.intern(SemanticType::Tuple(field_tys)));
                            }
                        }
                        if let Some(sym_id) = sym_id_opt {
                            let enum_ty = self.ctx.types.intern(SemanticType::Enum(sym_id, variant_tys));
                            self.ctx.tables.symbol_types.insert(sym_id, enum_ty);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Pass 2: Functions and global variables
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { params, return_type, .. } => {
                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();
                        let mut param_tys = Vec::new();
                        for param_id in params {
                            if let Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {
                                let param_ty = if let Some(t) = ty { self.lower_type(*t) } else { self.ctx.types.new_inference_var() };
                                param_tys.push(param_ty);
                            }
                        }
                        let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };
                        let func_ty = self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: ret_ty });
                        if let Some(sym_id) = sym_id_opt {
                            self.ctx.tables.symbol_types.insert(sym_id, func_ty);
                        }
                    }
                    Decl::Var { type_annot, .. } => {
                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();
                        let mut ty = self.ctx.types.new_inference_var();
                        if let Some(annot) = type_annot {
                            ty = self.lower_type(*annot);
                        }
                        if let Some(sym_id) = sym_id_opt {
                            self.ctx.tables.symbol_types.insert(sym_id, ty);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn lower_type(&mut self, ast_ty_id: mellis_ast::TypeId) -> SemanticTypeId {
        use mellis_ast::Type;
        let ast_ty = &self.arena.types[ast_ty_id.0 as usize];
        let sem_ty_id = match ast_ty {
            Type::Builtin(kind) => {
                let builtin = match kind {
                    BuiltinKind::I4 | BuiltinKind::I8 | BuiltinKind::I16 | BuiltinKind::I32 | BuiltinKind::I64 | BuiltinKind::I128 |
                    BuiltinKind::U4 | BuiltinKind::U8 | BuiltinKind::U16 | BuiltinKind::U32 | BuiltinKind::U64 | BuiltinKind::U128 => BuiltinType::Int,
                    BuiltinKind::F32 | BuiltinKind::F64 => BuiltinType::Float,
                    BuiltinKind::Bool => BuiltinType::Bool,
                    BuiltinKind::Str => BuiltinType::String,
                    BuiltinKind::Char => BuiltinType::Char,
                    BuiltinKind::Void => return self.ctx.types.intern(SemanticType::Void),
                };
                self.ctx.types.intern(SemanticType::Primitive(builtin))
            }
            Type::Named { segments, .. } => {
                let name = segments.last().map(|span| &self.source[span.start as usize..span.end as usize]);
                if let Some(symbol) = name.and_then(|name| self.ctx.symbol_table.lookup(name, crate::ScopeId(0))) {
                    self.ctx.tables.symbol_types.get(&symbol).copied().unwrap_or_else(|| self.ctx.types.new_inference_var())
                } else {
                    self.ctx.types.new_inference_var()
                }
            }
            Type::Tuple { elements } => {
                let mut elem_tys = Vec::new();
                for &e in elements {
                    elem_tys.push(self.lower_type(e));
                }
                self.ctx.types.intern(SemanticType::Tuple(elem_tys))
            }
            Type::Function { params, return_type, .. } => {
                let mut param_tys = Vec::new();
                for &p in params {
                    param_tys.push(self.lower_type(p));
                }
                let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };
                self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: ret_ty })
            }
            Type::Slice { inner } => {
                let inner_ty = self.lower_type(*inner);
                self.ctx.types.intern(SemanticType::Slice(inner_ty))
            }
            Type::Array { element_type, size } => {
                let inner_ty = self.lower_type(*element_type);
                let mut resolved_size = 0;
                if let mellis_ast::Expr::Literal(tok) = &self.arena.exprs[size.0 as usize] {
                    let text = &self.source[tok.span.start as usize..tok.span.end as usize];
                    if let Ok(val) = text.parse::<u64>() {
                        resolved_size = val;
                    }
                }
                self.ctx.types.intern(SemanticType::Array(inner_ty, resolved_size))
            }
            Type::Pointer { inner, .. } => {
                let inner_ty = self.lower_type(*inner);
                self.ctx.types.intern(SemanticType::Pointer(inner_ty))
            }
            _ => self.ctx.types.intern(SemanticType::Error),
        };
        self.ctx.tables.ast_type_to_semantic.insert(ast_ty_id, sem_ty_id);
        sem_ty_id
    }

    fn typecheck_item(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { body, .. } => {
                        if let Some(body_stmt) = body {
                            self.typecheck_stmt(body_stmt);
                        }
                    }
                    Decl::Var { initializer, pattern, .. } => {
                        let init_ty = if let Some(init) = initializer {
                            self.typecheck_expr(init)
                        } else {
                            self.ctx.types.new_inference_var()
                        };
                        
                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(decl_id).copied() {
                            let mut needs_update = false;
                            if let Some(current_ty) = self.ctx.tables.symbol_types.get(&sym_id).copied() {
                                let resolved = self.ctx.types.get(current_ty).clone();
                                if matches!(resolved, SemanticType::InferenceVar(_)) {
                                    needs_update = true;
                                }
                            }
                            if needs_update {
                                self.ctx.tables.symbol_types.insert(sym_id, init_ty);
                            }
                        }
                        
                        if let Some(pat_id) = pattern {
                            self.typecheck_pattern(pat_id, init_ty);
                        }
                    }
                    Decl::Extern { func, .. } => {
                        let item = Item::Decl(*func);
                        self.typecheck_item(&item);
                    }
                    _ => {}
                }
            }
            Item::Stmt(stmt_id) => {
                self.typecheck_stmt(stmt_id);
            }
        }
    }

    fn typecheck_pattern(&mut self, pat_id: &mellis_ast::PatId, ty: SemanticTypeId) {
        if let Some(sym_id) = self.ctx.tables.pat_symbols.get(pat_id).copied() {
            self.ctx.tables.symbol_types.insert(sym_id, ty);
        }
        self.ctx.tables.pat_types.insert(*pat_id, ty);
        
        let pattern = &self.arena.pats[pat_id.0 as usize];
        match pattern {
            mellis_ast::Pattern::Tuple { elements, .. } => {
                let resolved_ty = self.ctx.types.get(ty).clone();
                if let SemanticType::Tuple(elem_tys) = resolved_ty {
                    for (i, elem) in elements.iter().enumerate() {
                        if i < elem_tys.len() {
                            self.typecheck_pattern(elem, elem_tys[i]);
                        }
                    }
                }
            }
            mellis_ast::Pattern::Struct { fields, .. } => {
                let resolved_ty = self.ctx.types.get(ty).clone();
                if let SemanticType::Struct(sym_id, _) = resolved_ty {
                    let decl_id_opt = self.ctx.symbol_table.get_symbol(sym_id).decl_id;
                    if let Some(decl_id) = decl_id_opt {
                        let decl = self.arena.decls[decl_id.0 as usize].clone();
                        if let mellis_ast::Decl::Struct { fields: struct_fields, .. } = &decl {
                            for field in fields {
                                if let Some(field_pat) = field.pattern {
                                    let field_name_str = &self.source[field.name.start as usize..field.name.end as usize];
                                    
                                    let mut field_ty = self.ctx.types.new_inference_var();
                                    for struct_field in struct_fields {
                                        let struct_field_name = &self.source[struct_field.name.start as usize..struct_field.name.end as usize];
                                        if field_name_str == struct_field_name {
                                            field_ty = self.ctx.tables.ast_type_to_semantic.get(&struct_field.ty).copied().unwrap_or(field_ty);
                                            break;
                                        }
                                    }
                                    
                                    self.typecheck_pattern(&field_pat, field_ty);
                                }
                            }
                        }
                    }
                }
            }
            mellis_ast::Pattern::Enum { fields, .. } => {
                // TODO: enum variant extraction
                for field in fields {
                    let inf_var = self.ctx.types.new_inference_var();
                    self.typecheck_pattern(field, inf_var);
                }
            }
            _ => {}
        }
    }

    fn typecheck_stmt(&mut self, stmt_id: &mellis_ast::StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        match stmt {
            Stmt::Block { body, tail_expr } => {
                self.typecheck_items(body);
                if let Some(expr) = tail_expr {
                    self.typecheck_expr(expr);
                }
            }
            Stmt::Expr { expr, .. } => {
                self.typecheck_expr(expr);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_ty = self.typecheck_expr(condition);
                self.typecheck_stmt(then_branch);
                if let Some(else_br) = else_branch {
                    self.typecheck_stmt(else_br);
                }
                // Verify cond_ty is bool
            }
            Stmt::While { condition, body, .. } => {
                self.typecheck_expr(condition);
                self.typecheck_stmt(body);
            }
            Stmt::For { init, cond, step, body, iterable, .. } => {
                if let Some(item) = init { self.typecheck_item(item); }
                if let Some(c) = cond { self.typecheck_expr(c); }
                if let Some(s) = step { self.typecheck_expr(s); }
                if let Some(iter) = iterable { self.typecheck_expr(iter); }
                self.typecheck_stmt(body);
            }
            Stmt::Return { value } => {
                if let Some(val) = value {
                    self.typecheck_expr(val);
                }
            }
            Stmt::Unsafe { body } => {
                let old = self.is_unsafe_context;
                self.is_unsafe_context = true;
                self.typecheck_stmt(body);
                self.is_unsafe_context = old;
            }
            _ => {}
        }
    }

    fn typecheck_expr(&mut self, expr_id: &mellis_ast::ExprId) -> SemanticTypeId {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        let ty_id = match expr {
            Expr::Literal(tok) => {
                // Determine type based on literal token type
                let kind = match tok.kind {
                    TokenKind::IntegerLiteral | TokenKind::FloatLiteral => SemanticType::Primitive(BuiltinType::Int), // Simplified
                    TokenKind::StringLiteral => SemanticType::Primitive(BuiltinType::String),
                    TokenKind::KwTrue | TokenKind::KwFalse => SemanticType::Primitive(BuiltinType::Bool),
                    _ => SemanticType::Error,
                };
                self.ctx.types.intern(kind)
            }
            Expr::Identifier { .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {
                    if let Some(ty) = self.ctx.tables.symbol_types.get(sym_id) {
                        *ty
                    } else {
                        let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                        if let crate::SymbolKind::EnumVariant(_) = symbol.kind {
                            if let Some(decl_id) = symbol.decl_id {
                                if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {
                                    if let Some(ty) = self.ctx.tables.symbol_types.get(enum_sym_id) {
                                        *ty
                                    } else {
                                        self.ctx.types.new_inference_var()
                                    }
                                } else {
                                    self.ctx.types.new_inference_var()
                                }
                            } else {
                                self.ctx.types.new_inference_var()
                            }
                        } else {
                            self.ctx.types.new_inference_var()
                        }
                    }
                } else {
                    self.ctx.types.intern(SemanticType::Error)
                }
            }
            Expr::Binary { op, left, right, .. } => {
                let l_ty = self.typecheck_expr(left);
                let r_ty = self.typecheck_expr(right);
                
                let l_sem_ty = self.ctx.types.get(l_ty).clone();
                let r_sem_ty = self.ctx.types.get(r_ty).clone();
                
                if matches!(l_sem_ty, SemanticType::Pointer(_)) {
                    use mellis_ast::expr::BinaryOp;
                    if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                        if !self.is_unsafe_context {
                            self.ctx.diagnostics.push(mellis_common::Diagnostic::error("Pointer arithmetic requires an unsafe block."));
                        }
                    }
                }
                
                // For simplified logic: require left and right to be same, return left type
                let _ = self.unify(l_ty, r_ty); 
                l_ty 
            }
            Expr::Call { callee, args, .. } => {
                let callee_ty_id = self.typecheck_expr(callee);
                let mut ret_ty_id = self.ctx.types.new_inference_var();
                let callee_ty = self.ctx.types.get(callee_ty_id).clone();
                if let SemanticType::Function { params, return_type } = callee_ty {
                    ret_ty_id = return_type;
                }
                for arg in args {
                    self.typecheck_expr(&arg.value);
                }
                ret_ty_id
            }
            Expr::Assign { lvalue, value, .. } => {
                let l_ty = self.typecheck_expr(lvalue);
                let r_ty = self.typecheck_expr(value);
                let _ = self.unify(l_ty, r_ty);
                self.ctx.types.intern(SemanticType::Void)
            }
            Expr::Member { object, member } => {
                let obj_ty_id = self.typecheck_expr(object);
                let obj_ty = self.ctx.types.get(obj_ty_id).clone();
                if let SemanticType::Struct(sym_id, field_tys) = obj_ty {
                    if let Some(decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                        if let Decl::Struct { fields, .. } = &self.arena.decls[decl_id.0 as usize] {
                            let member_name = &self.source[member.start as usize..member.end as usize];
                            if let Some((index, _)) = fields.iter().enumerate().find(|(_, field)| {
                                &self.source[field.name.start as usize..field.name.end as usize] == member_name
                            }) {
                                return field_tys[index];
                            }
                            self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown field '{}'", member_name)).with_span(*member));
                            return self.ctx.types.new_inference_var();
                        }
                    }
                }
                self.ctx.diagnostics.push(Diagnostic::error("Member access requires a struct value").with_span(*member));
                self.ctx.types.new_inference_var()
            }
            Expr::StructInit { path, fields, .. } => {
                let Some(name_span) = path.last() else {
                    return self.ctx.types.new_inference_var();
                };
                let name = &self.source[name_span.start as usize..name_span.end as usize];
                let Some(symbol) = self.ctx.symbol_table.lookup(name, crate::ScopeId(0)) else {
                    self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown struct '{}'", name)).with_span(*name_span));
                    return self.ctx.types.new_inference_var();
                };
                let struct_ty = self.ctx.tables.symbol_types.get(&symbol).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());
                let SemanticType::Struct(_, field_tys) = self.ctx.types.get(struct_ty).clone() else {
                    self.ctx.diagnostics.push(Diagnostic::error(format!("'{}' is not a struct", name)).with_span(*name_span));
                    return self.ctx.types.new_inference_var();
                };
                let Some(decl_id) = self.ctx.tables.symbol_decls.get(&symbol).copied() else {
                    return struct_ty;
                };
                let Decl::Struct { fields: declared_fields, .. } = &self.arena.decls[decl_id.0 as usize] else {
                    return struct_ty;
                };
                for field in fields {
                    let field_name = &self.source[field.name.start as usize..field.name.end as usize];
                    if let Some((index, _)) = declared_fields.iter().enumerate().find(|(_, declared)| {
                        &self.source[declared.name.start as usize..declared.name.end as usize] == field_name
                    }) {
                        let value_ty = self.typecheck_expr(&field.value);
                        if self.unify(field_tys[index], value_ty).is_err() {
                            self.ctx.diagnostics.push(Diagnostic::error(format!("Type mismatch for field '{}'", field_name)).with_span(field.name));
                        }
                    } else {
                        self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown field '{}' for struct '{}'", field_name, name)).with_span(field.name));
                        self.typecheck_expr(&field.value);
                    }
                }
                struct_ty
            }
            Expr::ArrayLiteral { elements } => {
                let mut elem_ty = self.ctx.types.new_inference_var();
                for (i, el) in elements.iter().enumerate() {
                    let ty = self.typecheck_expr(el);
                    if i == 0 {
                        elem_ty = ty;
                    } else if self.unify(elem_ty, ty).is_err() {
                        self.ctx.diagnostics.push(Diagnostic::error("Array literal elements must have the same type"));
                    }
                }
                self.ctx.types.intern(SemanticType::Array(elem_ty, elements.len() as u64))
            }
            Expr::TupleLiteral { elements } => {
                let mut elem_tys = Vec::new();
                for el in elements {
                    elem_tys.push(self.typecheck_expr(el));
                }
                self.ctx.types.intern(SemanticType::Tuple(elem_tys))
            }
            Expr::TupleIndex { object, index } => {
                let base_ty_id = self.typecheck_expr(object);
                let base_ty = self.ctx.types.get(base_ty_id).clone();
                match base_ty {
                    SemanticType::Tuple(elem_tys) => {
                        if *index as usize >= elem_tys.len() {
                            self.ctx.diagnostics.push(Diagnostic::error(format!("Tuple index {} out of bounds (tuple has {} elements)", index, elem_tys.len())));
                            self.ctx.types.new_inference_var()
                        } else {
                            elem_tys[*index as usize]
                        }
                    }
                    _ => {
                        self.ctx.diagnostics.push(Diagnostic::error("Cannot index into a non-tuple type"));
                        self.ctx.types.new_inference_var()
                    }
                }
            }
            Expr::Index { base, index } => {
                let base_ty_id = self.typecheck_expr(base);
                self.typecheck_expr(index);
                
                let base_ty = self.ctx.types.get(base_ty_id).clone();
                match base_ty {
                    SemanticType::Array(el, _) => el,
                    SemanticType::Slice(el) => el,
                    _ => self.ctx.types.new_inference_var()
                }
            }
            Expr::MethodCall { object, method_name, args, .. } => {
                let obj_ty_id = self.typecheck_expr(object);
                let obj_ty = self.ctx.types.get(obj_ty_id).clone();
                eprintln!("DEBUG MethodCall obj_ty: {:?}", obj_ty);
                
                for arg in args {
                    self.typecheck_expr(&arg.value);
                }

                let name_str = &self.source[method_name.start as usize..method_name.end as usize];
                if matches!(obj_ty, SemanticType::Pointer(_)) {
                    if name_str == "add" || name_str == "sub" || name_str == "offset" {
                        if !self.is_unsafe_context {
                            self.ctx.diagnostics.push(mellis_common::Diagnostic::error("Pointer arithmetic requires an unsafe block.").with_span(*method_name));
                        }
                    }
                }
                
                self.ctx.types.new_inference_var()
            }
            Expr::Cast { expr: e, target_type } => {
                self.typecheck_expr(e);
                self.lower_type(*target_type)
            }
            Expr::Match { subject, arms } => {
                let subject_ty = self.typecheck_expr(subject);
                let mut result_ty = self.ctx.types.new_inference_var();
                for (i, arm) in arms.iter().enumerate() {
                    self.typecheck_pattern(&arm.pattern, subject_ty);
                    // Typecheck arm body
                    self.typecheck_stmt(&arm.body);
                    // Use first arm's type as the result type
                    if i == 0 {
                        match &self.arena.stmts[arm.body.0 as usize] {
                            Stmt::Block { tail_expr: Some(tail), .. } => {
                                result_ty = self.typecheck_expr(tail);
                            }
                            Stmt::Expr { expr, has_semicolon: false } => {
                                result_ty = self.typecheck_expr(expr);
                            }
                            _ => {}
                        }
                    }
                }
                result_ty
            }
            Expr::Lambda { body, params, .. } => {
                for param_id in params {
                    // params already resolved
                }
                self.typecheck_stmt(body);
                self.ctx.types.new_inference_var()
            }
            Expr::Try { expr: e } => {
                self.typecheck_expr(e)
            }
            Expr::Unary { operand, .. } => {
                self.typecheck_expr(operand)
            }
            _ => self.ctx.types.intern(SemanticType::Error)
        };
        
        self.ctx.tables.expr_types.insert(*expr_id, ty_id);
        ty_id
    }
}
