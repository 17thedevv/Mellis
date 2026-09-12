use crate::mvir::*;
use luna_ast::{AstArena, Item, Expr, Stmt, Decl};
use luna_semantic::SemanticContext;
use std::collections::HashMap;

pub struct MvirGenerator<'a> {
    arena: &'a AstArena,
    ctx: &'a SemanticContext,
    source_manager: &'a luna_common::source::SourceManager,
    module: Module,
    
    // State during function generation
    current_function: Option<Function>,
    current_block: Option<BasicBlock>,
    next_label_id: u32,
    
    // Track local variables to their Alloca ValueId
    locals: HashMap<luna_common::ids::SymbolId, ValueId>,
    lexical_scopes: Vec<Vec<luna_common::ids::SymbolId>>,
    loop_scopes: Vec<usize>,
    loop_break_targets: Vec<LabelId>,
    loop_continue_targets: Vec<LabelId>,
    current_span: Option<luna_common::Span>,
    current_async_future: Option<ValueId>,
    pub diagnostics: Vec<luna_common::Diagnostic>,
    
    // The current monomorphic instance being generated
    current_instance: Option<*const luna_semantic::mono::InstantiatedFunction>,
}

impl<'a> MvirGenerator<'a> {

    pub fn get_span_text(&self, span: luna_common::ids::Span) -> &'a str {
        &self.source_manager.get_file(span.file_id).unwrap().source[span.start as usize..span.end as usize]
    }

    pub fn new(arena: &'a AstArena, ctx: &'a SemanticContext, source_manager: &'a luna_common::source::SourceManager) -> Self {
        Self {
            arena,
            ctx,
            source_manager,
            module: Module::new(),
            current_function: None,
            current_block: None,
            next_label_id: 0,
            locals: HashMap::new(),
            lexical_scopes: Vec::new(),
            loop_scopes: Vec::new(),
            loop_break_targets: Vec::new(),
            loop_continue_targets: Vec::new(),
            current_span: None,
            current_async_future: None,
            diagnostics: Vec::new(),
            current_instance: None,
        }
    }

    pub fn generate(mut self, _items: &[Item]) -> (Module, Vec<luna_common::Diagnostic>) {
        for instance in &self.ctx.instantiated_functions {
            self.generate_mono_instance(instance);
        }
        (self.module, self.diagnostics)
    }

    pub fn generate_function_by_decl_id(&mut self, decl_id: luna_ast::DeclId) {
        let instance = luna_semantic::mono::InstantiatedFunction {
            instance: luna_semantic::mono::MonoInstance {
                decl_id,
                subst: Vec::new(),
                closure_id: None,
            },
            expr_types: std::collections::HashMap::new(),
            symbol_types: std::collections::HashMap::new(),
            pat_types: std::collections::HashMap::new(),
            mono_calls: std::collections::HashMap::new(),
            mono_for_loops: std::collections::HashMap::new(),
            closure_capture_bindings: Vec::new(),
            closure_env_type: None,
            closure_env_ptr_type: None,
        };
        self.generate_mono_instance(&instance);
    }
    
    fn get_symbol_type(&self, sym_id: &luna_common::ids::SymbolId) -> Option<luna_semantic::SemanticTypeId> {
        if let Some(inst_ptr) = self.current_instance {
            let inst = unsafe { &*inst_ptr };
            if let Some(&ty) = inst.symbol_types.get(sym_id) {
                return Some(ty);
            }
        }
        self.ctx.tables.symbol_types.get(sym_id).copied()
    }
    
    fn get_pat_type(&self, pat_id: &luna_ast::PatId) -> Option<luna_semantic::SemanticTypeId> {
        if let Some(inst_ptr) = self.current_instance {
            let inst = unsafe { &*inst_ptr };
            if let Some(&ty) = inst.pat_types.get(pat_id) {
                return Some(ty);
            }
        }
        self.ctx.tables.pat_types.get(pat_id).copied()
    }

    fn resolve_ast_type(&self, type_id: &luna_ast::TypeId) -> luna_semantic::SemanticTypeId {
        if let Some(&ty) = self.ctx.tables.ast_type_to_semantic.get(type_id) {
            if ty != luna_semantic::SemanticTypeId(0) {
                if let Some(inst_ptr) = self.current_instance {
                    let inst = unsafe { &*inst_ptr };
                    if !inst.instance.subst.is_empty() {
                        let mut subst = luna_semantic::ty::Substitution::new();
                        for &(sym, t) in &inst.instance.subst {
                            subst.insert(sym, t);
                        }
                        if let luna_semantic::SemanticType::GenericParam(sym) = self.ctx.types.get(ty) {
                            if let Some(&new_ty) = subst.get(*sym) {
                                return new_ty;
                            }
                        }
                    }
                }
                return ty;
            }
        }
        
        if (type_id.0 as usize) < self.arena.types.len() {
            match &self.arena.types[type_id.0 as usize] {
                luna_ast::Type::Builtin(bk) => {
                    use luna_lexer::BuiltinKind;
                    use luna_semantic::ty::BuiltinType;
                    let b_ty = match bk {
                        BuiltinKind::I8 => BuiltinType::I8,
                        BuiltinKind::U8 => BuiltinType::U8,
                        BuiltinKind::I16 => BuiltinType::I16,
                        BuiltinKind::U16 => BuiltinType::U16,
                        BuiltinKind::I32 => BuiltinType::I32,
                        BuiltinKind::U32 => BuiltinType::U32,
                        BuiltinKind::I64 => BuiltinType::I64,
                        BuiltinKind::U64 => BuiltinType::U64,
                        BuiltinKind::I128 => BuiltinType::I128,
                        BuiltinKind::U128 => BuiltinType::U128,
                        BuiltinKind::Isize => BuiltinType::Isize,
                        BuiltinKind::Usize => return self.ctx.types.usize_id(),
                        BuiltinKind::F32 => BuiltinType::F32,
                        BuiltinKind::F64 => BuiltinType::F64,
                        BuiltinKind::Bool => return self.ctx.types.bool_id(),
                        BuiltinKind::Char => BuiltinType::Char,
                        BuiltinKind::Str => BuiltinType::String,
                        BuiltinKind::Void => return luna_semantic::SemanticTypeId(0),
                    };
                    for id in self.ctx.types.get_all_types() {
                        if let luna_semantic::SemanticType::Primitive(p) = self.ctx.types.get(id) {
                            if *p == b_ty {
                                return id;
                            }
                        }
                    }
                }
                luna_ast::Type::Pointer { is_mutable, inner } => {
                    let inner_sem = self.resolve_ast_type(inner);
                    let is_rw = if *is_mutable { luna_semantic::ty::Mutability::Mutable } else { luna_semantic::ty::Mutability::Immutable };
                    for id in self.ctx.types.get_all_types() {
                        if let luna_semantic::SemanticType::Pointer(m, elem) = self.ctx.types.get(id) {
                            if *m == is_rw && *elem == inner_sem {
                                return id;
                            }
                        }
                    }
                }
                luna_ast::Type::Reference { is_mutable, inner, .. } => {
                    let inner_sem = self.resolve_ast_type(inner);
                    let is_rw = if *is_mutable { luna_semantic::ty::Mutability::Mutable } else { luna_semantic::ty::Mutability::Immutable };
                    for id in self.ctx.types.get_all_types() {
                        if let luna_semantic::SemanticType::Reference(_, m, elem) = self.ctx.types.get(id) {
                            if *m == is_rw && *elem == inner_sem {
                                return id;
                            }
                        }
                    }
                }
                luna_ast::Type::Named { segments, .. } => {
                    if let Some(first) = segments.first() {
                        let name = self.get_span_text(*first);
                        if let Some(inst_ptr) = self.current_instance {
                            let inst = unsafe { &*inst_ptr };
                            for &(sym, t) in &inst.instance.subst {
                                let sym_name = &self.ctx.symbol_table.get_symbol(sym).name;
                                if sym_name == name {
                                    return t;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        luna_semantic::SemanticTypeId(0)
    }

    pub fn generate_all_known_functions(&mut self) {
        for instance in &self.ctx.instantiated_functions {
            self.generate_mono_instance(instance);
        }
        for (decl_id_idx, decl) in self.arena.decls.iter().enumerate() {
            if let luna_ast::Decl::Function { generic_params, body: Some(_), .. } = decl {
                if generic_params.is_empty() {
                    let decl_id = luna_ast::DeclId(decl_id_idx as u32);
                    let sym_id_opt = self.ctx.tables.decl_symbols.get(&decl_id).copied();
                    let fn_name = sym_id_opt.map(|s| {
                        if (s.0 as usize) < self.ctx.symbol_table.symbols.len() {
                            self.ctx.symbol_table.symbols[s.0 as usize].name.clone()
                        } else {
                            String::new()
                        }
                    }).unwrap_or_default();
                    if !fn_name.is_empty() && !self.module.functions.iter().any(|f| f.name.name == fn_name) {
                        self.generate_function_by_decl_id(decl_id);
                    }
                }
            }
        }
    }

    pub fn current_module(&self) -> &Module {
        &self.module
    }

    pub fn generate_expr_as_function(&mut self, expr_id: &luna_ast::ExprId, ret_ty: luna_semantic::SemanticTypeId) -> Function {
        let global_id = GlobalId {
            name: format!("__comptime_eval_{}", expr_id.0),
            symbol_id: None,
        };
        self.current_function = Some(Function {
            name: global_id,
            is_extern: false,
            is_async: false,
            arg_count: 0,
            link_name: None,
            param_types: Vec::new(),
            ret_ty,
            blocks: Vec::new(),
            values: Vec::new(),
        });
        self.start_block(LabelId { name: "entry".to_string() });
        let val_op = self.generate_expr(expr_id);
        self.terminate_block(Terminator::Ret { value: Some(val_op) });
        if let Some(block) = self.current_block.take() {
            self.current_function.as_mut().unwrap().blocks.push(block);
        }
        self.current_function.take().unwrap()
    }

    pub fn generate_stmt_as_function(&mut self, stmt_id: &luna_ast::StmtId, ret_ty: luna_semantic::SemanticTypeId) -> Function {
        let global_id = GlobalId {
            name: format!("__comptime_eval_stmt_{}", stmt_id.0),
            symbol_id: None,
        };
        self.current_function = Some(Function {
            name: global_id,
            is_extern: false,
            is_async: false,
            arg_count: 0,
            link_name: None,
            param_types: Vec::new(),
            ret_ty,
            blocks: Vec::new(),
            values: Vec::new(),
        });
        self.start_block(LabelId { name: "entry".to_string() });
        let ret_val = self.generate_block_expr(stmt_id);
        if let Some(mut block) = self.current_block.take() {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator::Ret { value: Some(ret_val) });
            }
            self.current_function.as_mut().unwrap().blocks.push(block);
        }
        self.current_function.take().unwrap()
    }

    fn push_scope(&mut self) {
        self.lexical_scopes.push(Vec::new());
    }

    fn pop_scope_and_drop(&mut self, keep: Option<luna_common::ids::SymbolId>) {
        if let Some(scope) = self.lexical_scopes.pop() {
            self.emit_drops_for_scope(&scope, keep);
        }
    }

    fn emit_drops_for_scope(&mut self, scope: &[luna_common::ids::SymbolId], keep: Option<luna_common::ids::SymbolId>) {
        for &sym in scope.iter().rev() {
            if Some(sym) == keep { continue; }
            if let Some(&val_id) = self.locals.get(&sym) {
                let ty_id = self.get_symbol_type(&sym).unwrap_or(luna_semantic::SemanticTypeId(0));
                if self.ctx.needs_drop(ty_id) {
                    let callee = match self.ctx.types.get(ty_id) {
                        luna_semantic::SemanticType::Struct(sym_id, ..) => {
                            self.ctx.tables.drop_impls.get(sym_id).map(|&meth_sym| {
                                let name = self.ctx.symbol_table.get_symbol(meth_sym).name.clone();
                                GlobalId { name, symbol_id: Some(meth_sym) }
                            })
                        }
                        _ => None,
                    };
                    self.push_inst(Instruction::Drop { value: Operand::Value(val_id), callee, ty: ty_id }, ty_id);
                }
            }
        }
    }

    fn emit_drops_up_to(&mut self, target_depth: usize, keep: Option<luna_common::ids::SymbolId>) {
        for i in (target_depth..self.lexical_scopes.len()).rev() {
            let scope = self.lexical_scopes[i].clone();
            self.emit_drops_for_scope(&scope, keep);
        }
    }

    fn generate_item(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Var { name, initializer, pattern, .. } => {
                        let init_op = initializer.as_ref().map(|init_expr| self.generate_expr(init_expr));
                        
                        let prev_span = self.current_span.clone();
                        self.current_span = Some(*name);
                        
                        if let Some(pat_id) = pattern {
                            self.bind_pattern(pat_id, init_op);
                        }
                        
                        self.current_span = prev_span;
                    }
                    _ => unreachable!("ICE: Unhandled variant, should be impossible after semantic invariants") // Functions are handled by generate_mono_instance
                }
            }
            Item::Stmt(stmt_id) => {
                self.generate_stmt(stmt_id);
            }
        }
    }

    fn bind_pattern(&mut self, pat_id: &luna_ast::PatId, val_op: Option<Operand>) {
        match &self.arena.pats[pat_id.0 as usize] {
            luna_ast::Pattern::Identifier { .. } => {
                if let Some(sym_id) = self.ctx.tables.pat_symbols.get(pat_id).copied() {
                    let mut ty_id = self.get_symbol_type(&sym_id).unwrap_or(luna_semantic::SemanticTypeId(0));
                    if ty_id == luna_semantic::SemanticTypeId(0) {
                        if let Some(Operand::Value(v)) = &val_op {
                            ty_id = self.current_function.as_ref().unwrap().values[v.0 as usize].ty;
                        }
                    }
                    let alloca_val = self.push_inst(Instruction::Alloca, ty_id);
                    self.locals.insert(sym_id, alloca_val);
                    if let Some(scope) = self.lexical_scopes.last_mut() {
                        scope.push(sym_id);
                    }
                    
                    if let Some(val) = val_op {
                        self.push_inst(Instruction::Store {
                            ptr: Operand::Value(alloca_val),
                            value: val,
                        }, ty_id);
                    }
                }
            }
            luna_ast::Pattern::Struct { fields, .. } => {
                let ty_id = self.ctx.tables.pat_types.get(pat_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                let resolved_ty = self.ctx.types.get(ty_id).clone();
                if let luna_semantic::SemanticType::Struct(sym_id, _, _) = resolved_ty {
                    if let Some(decl_id) = self.ctx.symbol_table.get_symbol(sym_id).decl_id {
                        let decl = self.arena.decls[decl_id.0 as usize].clone();
                        if let luna_ast::Decl::Struct { fields: struct_fields, .. } = &decl {
                            for field in fields {
                                if let Some(field_pat) = field.pattern {
                                    let field_name_str = self.get_span_text(field.name);
                                    let mut field_idx = 0;
                                    for (idx, struct_field) in struct_fields.iter().enumerate() {
                                        let struct_field_name = self.get_span_text(struct_field.name);
                                        if field_name_str == struct_field_name {
                                            field_idx = idx;
                                            break;
                                        }
                                    }
                                    let field_ty = self.ctx.tables.pat_types.get(&field_pat).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                                    let extract_op = val_op.as_ref().map(|v| {
                                        let extract_val = self.push_inst(Instruction::Extract {
                                            value: v.clone(),
                                            variant_idx: 0,
                                            field_idx: field_idx as u32,
                                        }, field_ty);
                                        Operand::Value(extract_val)
                                    });
                                    self.bind_pattern(&field_pat, extract_op);
                                }
                            }
                        }
                    }
                }
            }
            luna_ast::Pattern::Tuple { elements, .. } => {
                for (idx, elem) in elements.iter().enumerate() {
                    let field_ty = self.ctx.tables.pat_types.get(elem).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                    let extract_op = val_op.as_ref().map(|v| {
                        let extract_val = self.push_inst(Instruction::Extract {
                            value: v.clone(),
                            variant_idx: 0,
                            field_idx: idx as u32,
                        }, field_ty);
                        Operand::Value(extract_val)
                    });
                    self.bind_pattern(elem, extract_op);
                }
            }
            luna_ast::Pattern::Enum { fields: elements, path } => {
                let mut variant_idx = 0;
                if let Some(name_span) = path.last() {
                    let name_str = self.get_span_text(*name_span);
                    for symbol in &self.ctx.symbol_table.symbols {
                        if let luna_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                            if symbol.name.ends_with(name_str) {
                                variant_idx = idx;
                                break;
                            }
                        }
                    }
                }
                
                for (field_idx, elem) in elements.iter().enumerate() {
                    let field_ty = self.ctx.tables.pat_types.get(elem).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                    let extract_op = val_op.as_ref().map(|v| {
                        let extract_val = self.push_inst(Instruction::Extract {
                            value: v.clone(),
                            variant_idx,
                            field_idx: field_idx as u32,
                        }, field_ty);
                        Operand::Value(extract_val)
                    });
                    self.bind_pattern(elem, extract_op);
                }
            }
            _ => unreachable!("ICE: Unhandled variant, should be impossible after semantic invariants")
        }
    }

    fn generate_lvalue(&mut self, expr_id: &luna_ast::ExprId) -> Operand {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        
        match expr {
            Expr::Identifier { .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id).copied() {
                    if let Some(&val_id) = self.locals.get(&sym_id) {
                        return Operand::Value(val_id);
                    }
                }
                Operand::Number("0".to_string())
            }
            Expr::Member { object, member } => {
                let mut base_op = self.generate_lvalue(object);
                let obj_ty_id = self.ctx.tables.expr_types.get(object).copied()
                    .or_else(|| self.ctx.tables.expr_symbols.get(object).and_then(|sym| self.get_symbol_type(sym)))
                    .unwrap_or(luna_semantic::SemanticTypeId(0));
                let mut struct_sym_opt = None;
                let mut struct_field_tys_opt = None;
                match self.ctx.types.get(obj_ty_id) {
                    luna_semantic::SemanticType::Struct(sym_id, _, field_tys) => {
                        struct_sym_opt = Some(*sym_id);
                        struct_field_tys_opt = Some(field_tys.clone());
                    }
                    luna_semantic::SemanticType::Reference(_, _, inner) | luna_semantic::SemanticType::Pointer(_, inner) => {
                        let load_val = self.push_inst(Instruction::Load { ptr: base_op }, obj_ty_id);
                        base_op = Operand::Value(load_val);
                        if let luna_semantic::SemanticType::Struct(sym_id, _, field_tys) = self.ctx.types.get(*inner) {
                            struct_sym_opt = Some(*sym_id);
                            struct_field_tys_opt = Some(field_tys.clone());
                        }
                    }
                    _ => {}
                }
                let mut field_idx = 0;
                
                if let Some(sym_id) = struct_sym_opt {
                    let sym = self.ctx.symbol_table.get_symbol(sym_id);
                    if let Some(decl_id) = sym.decl_id {
                        if let luna_ast::Decl::Struct { fields, .. } = &self.arena.decls[decl_id.0 as usize] {
                            let member_name = self.get_span_text(*member).to_string();
                            for (i, f) in fields.iter().enumerate() {
                                let f_name = self.get_span_text(f.name).to_string();
                                if f_name == member_name {
                                    field_idx = i as u32;
                                    break;
                                }
                            }
                        }
                    }
                } else if let Some(&idx) = self.ctx.tables.expr_member_indices.get(expr_id) {
                    field_idx = idx;
                }
                
                let mut field_ty = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                if field_ty == luna_semantic::SemanticTypeId(0) {
                    if let Some(field_tys) = &struct_field_tys_opt {
                        if let Some(&fty) = field_tys.get(field_idx as usize) {
                            field_ty = fty;
                        }
                    }
                }
                let field_ptr_val = self.push_inst(Instruction::FieldPtr {
                    base: base_op,
                    field_idx,
                }, field_ty);
                Operand::Value(field_ptr_val)
            }
            Expr::TupleIndex { object, index } => {
                let mut base_op = self.generate_lvalue(object);
                let obj_ty_id = self.ctx.tables.expr_types.get(object).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                let mut tuple_field_tys_opt = None;
                match self.ctx.types.get(obj_ty_id) {
                    luna_semantic::SemanticType::Tuple(types) => {
                        tuple_field_tys_opt = Some(types.clone());
                    }
                    luna_semantic::SemanticType::Reference(_, _, inner) | luna_semantic::SemanticType::Pointer(_, inner) => {
                        let load_val = self.push_inst(Instruction::Load { ptr: base_op }, obj_ty_id);
                        base_op = Operand::Value(load_val);
                        if let luna_semantic::SemanticType::Tuple(types) = self.ctx.types.get(*inner) {
                            tuple_field_tys_opt = Some(types.clone());
                        }
                    }
                    _ => {}
                }
                let mut field_ty = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                if field_ty == luna_semantic::SemanticTypeId(0) {
                    if let Some(types) = &tuple_field_tys_opt {
                        if let Some(&fty) = types.get(*index as usize) {
                            field_ty = fty;
                        }
                    }
                }
                let field_ptr_val = self.push_inst(Instruction::FieldPtr {
                    base: base_op,
                    field_idx: *index,
                }, field_ty);
                Operand::Value(field_ptr_val)
            }
            Expr::Unary { op: luna_ast::expr::UnaryOp::Deref, operand } |
            Expr::Unary { op: luna_ast::expr::UnaryOp::DerefMut, operand } => {
                self.generate_expr(operand)
            }
            _ => Operand::Number("0".to_string()),
        }
    }

    fn generate_closure_mono_instance(&mut self, instance: &luna_semantic::mono::InstantiatedFunction, expr_id: luna_ast::ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        if let Expr::Lambda { body, params, return_type: _, is_move: _ } = expr {
            let mut fn_name = format!("closure_{}", expr_id.0);
            if !instance.instance.subst.is_empty() {
                fn_name = format!("{}_mono", fn_name);
            }
            let global_id = GlobalId {
                name: fn_name,
                symbol_id: None, // No symbol for anonymous closure
            };
            
            // The environment is passed as the first parameter to the closure (the environment pointer)
            let env_ty_id = instance.closure_env_ptr_type
                .or_else(|| self.ctx.tables.closure_env_ptr_types.get(&expr_id).copied())
                .unwrap_or(luna_semantic::SemanticTypeId(0));
            
            let mut ret_ty_id = luna_semantic::SemanticTypeId(0);
            if let Some(luna_semantic::SemanticType::Closure(_, _, ret)) = instance.expr_types.get(&expr_id).map(|id| self.ctx.types.get(*id)) {
                ret_ty_id = *ret;
            } else if let Some(luna_semantic::SemanticType::Closure(_, _, ret)) = self.ctx.tables.expr_types.get(&expr_id).map(|id| self.ctx.types.get(*id)) {
                ret_ty_id = *ret;
            }

            let mut param_types = Vec::new();
            if let Some(env_ptr_ty) = instance.closure_env_ptr_type {
                param_types.push(env_ptr_ty);
            } else {
                param_types.push(luna_semantic::SemanticTypeId(0));
            }
            for param_id in params {
                let mut param_ty = luna_semantic::SemanticTypeId(0);
                if let Some(sym_id) = self.ctx.tables.decl_symbols.get(param_id).copied() {
                    param_ty = self.get_symbol_type(&sym_id).unwrap_or(luna_semantic::SemanticTypeId(0));
                }
                param_types.push(param_ty);
            }
            
            self.current_function = Some(Function {
                name: global_id,
                is_extern: false,
                is_async: false,
                ret_ty: ret_ty_id,
                arg_count: params.len() + 1, // environment is the extra argument (first)
                link_name: None,
                param_types,
                blocks: Vec::new(),
                values: Vec::new(),
            });
            self.locals.clear();
            self.lexical_scopes.clear();
            self.loop_scopes.clear();
            self.loop_break_targets.clear();
            self.loop_continue_targets.clear();
            self.push_scope(); // Function root scope
            self.next_label_id = 0;
            
            let entry_label = self.new_label("entry");
            self.start_block(entry_label);
            
            // The first values must be allocas for the hidden environment and explicit parameters.
            let env_param = self.push_inst(Instruction::Alloca, env_ty_id);
            self.current_function.as_mut().unwrap().values[env_param.0 as usize].origin = ValueOrigin::Parameter(0);
            for (idx, param_id) in params.iter().enumerate() {
                if let Decl::Param { .. } = &self.arena.decls[param_id.0 as usize] {
                    if let Some(sym_id) = self.ctx.tables.decl_symbols.get(&param_id).copied() {
                        let ty_id = instance.symbol_types.get(&sym_id).copied()
                            .or_else(|| self.ctx.tables.symbol_types.get(&sym_id).copied())
                            .unwrap_or(luna_semantic::SemanticTypeId(0));
                        let alloc_val = self.push_inst(Instruction::Alloca, ty_id);
                        self.current_function.as_mut().unwrap().values[alloc_val.0 as usize].origin = ValueOrigin::Parameter((idx + 1) as u32);
                        self.locals.insert(sym_id, alloc_val);
                        self.lexical_scopes.last_mut().unwrap().push(sym_id);
                    }
                }
            }

            // Bind captured variables to environment fields after parameter allocas exist.
            let closure_bindings = if instance.closure_capture_bindings.is_empty() {
                self.ctx.tables.expect_closure_capture_bindings(expr_id)
            } else {
                instance.closure_capture_bindings.clone()
            };
            if !closure_bindings.is_empty() {
                for binding in &closure_bindings {
                    let field_ptr = self.push_inst(Instruction::FieldPtr {
                        base: Operand::Value(env_param),
                        field_idx: binding.env_field,
                    }, binding.env_ty);
                    let val = match binding.mode {
                        luna_semantic::semantic_tables::CaptureMode::SharedBorrow |
                        luna_semantic::semantic_tables::CaptureMode::MutableBorrow => {
                            self.push_inst(Instruction::Load { ptr: Operand::Value(field_ptr) }, binding.env_ty)
                        }
                        luna_semantic::semantic_tables::CaptureMode::Move => field_ptr,
                    };
                    self.locals.insert(binding.symbol, val);
                }
            }
                        
            self.generate_fn_body(body, ret_ty_id);

        if let Some(func) = self.current_function.take() {
            self.module.functions.push(func);
        }
    }
}

    fn generate_mono_instance(&mut self, instance: &luna_semantic::mono::InstantiatedFunction) {
        // Monomorphization Barrier (P0-C / Rule 8):
        // Assert that all types inside the instantiated function are strictly monomorphic.
        // No GenericParam, InferenceVar, Error, or Unresolved Projection may enter MVIR lowering.
        for (&sym_id, &ty) in &instance.symbol_types {
            if !self.ctx.types.is_monomorphic(ty) {
                panic!("MVIR Monomorphization Barrier Violation: Symbol {:?} has non-monomorphic type {:?}", sym_id, self.ctx.types.get(ty));
            }
        }
        for (&expr_id, &ty) in &instance.expr_types {
            if !self.ctx.types.is_monomorphic(ty) {
                // Expressions referencing uncalled generic functions or enum types are exempted
                if !matches!(self.ctx.types.get(ty), luna_semantic::SemanticType::Function { .. } | luna_semantic::SemanticType::Enum(..)) {
                    panic!("MVIR Monomorphization Barrier Violation: Expr {:?} has non-monomorphic type {:?}", expr_id, self.ctx.types.get(ty));
                }
            }
        }

        self.current_instance = Some(instance as *const _);
        
        if let Some(closure_id) = instance.instance.closure_id {
            self.generate_closure_mono_instance(instance, closure_id);
            self.current_instance = None;
            return;
        }
        if (instance.instance.decl_id.0 as usize) >= self.arena.decls.len() {
            let sym_id_opt = self.ctx.tables.decl_symbols.get(&instance.instance.decl_id).copied();
            let mut fn_name = "func".to_string();
            let mut ret_ty_id = luna_semantic::SemanticTypeId(0);
            let mut param_types = Vec::new();
            if let Some(sym_id) = sym_id_opt {
                if (sym_id.0 as usize) < self.ctx.symbol_table.symbols.len() {
                    fn_name = self.ctx.symbol_table.symbols[sym_id.0 as usize].name.clone();
                }
                if let Some(fn_ty_id) = self.get_symbol_type(&sym_id) {
                    if let luna_semantic::SemanticType::Function { return_type, params } = self.ctx.types.get(fn_ty_id) {
                        ret_ty_id = *return_type;
                        param_types = params.clone();
                    }
                }
            }
            let global_id = GlobalId {
                name: fn_name,
                symbol_id: sym_id_opt,
            };
            let func = Function {
                name: global_id,
                is_extern: true,
                is_async: false,
                arg_count: param_types.len(),
                link_name: None,
                param_types,
                ret_ty: ret_ty_id,
                blocks: Vec::new(),
                values: Vec::new(),
            };
            self.module.functions.push(func);
            self.current_instance = None;
            return;
        }
        let decl = &self.arena.decls[instance.instance.decl_id.0 as usize];
        if let Decl::Function { body, .. } = decl {
            let sym_id_opt = self.ctx.tables.decl_symbols.get(&instance.instance.decl_id).copied();
            let mut fn_name = "func".to_string();
            let mut ret_ty_id = luna_semantic::SemanticTypeId(0);
            
            let mut resolved_param_types = None;
            let mut link_name = None;

            if let Decl::Function { annotations, .. } = decl {
                for annot in annotations {
                    let annot_name_str = if (annot.name.end as usize) <= self.source_manager.get_file(annot.name.file_id).unwrap().source.len() && annot.name.start <= annot.name.end {
                        self.get_span_text(annot.name)
                    } else { "" };

                    if annot_name_str == "link" {
                        for arg in &annot.args {
                            if let Some(key_span) = arg.key {
                                let key_str = if (key_span.end as usize) <= self.source_manager.get_file(key_span.file_id).unwrap().source.len() && key_span.start <= key_span.end {
                                    self.get_span_text(key_span)
                                } else { "" };

                                if key_str == "name" {
                                    if let luna_ast::Expr::Literal(_, val) = &self.arena.exprs[arg.value.0 as usize] {
                                        link_name = Some(val.trim_matches('"').to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(sym_id) = sym_id_opt {
                if (sym_id.0 as usize) < self.ctx.symbol_table.symbols.len() {
                    fn_name = self.ctx.symbol_table.symbols[sym_id.0 as usize].name.clone();
                }
                if let Some(fn_ty_id) = self.get_symbol_type(&sym_id) {
                    if let luna_semantic::SemanticType::Function { return_type, params } = self.ctx.types.get(fn_ty_id) {
                        ret_ty_id = *return_type;
                        resolved_param_types = Some(params.clone());
                    }
                }
            }
            
            let mut suffix = String::new();
            if !instance.instance.subst.is_empty() {
                suffix = "_mono".to_string();
            }
            let name_str = format!("{}{}", fn_name, suffix);
            let global_id = GlobalId {
                name: name_str,
                symbol_id: sym_id_opt,
            };
            
            let arg_count = if let Decl::Function { params, .. } = decl { params.len() } else { 0 };
            let param_types = resolved_param_types.unwrap_or_else(|| {
                let mut p = Vec::new();
                if let Decl::Function { params, .. } = decl {
                    for param_id in params {
                        let mut param_ty = luna_semantic::SemanticTypeId(0);
                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(param_id).copied() {
                            param_ty = self.get_symbol_type(&sym_id).unwrap_or(luna_semantic::SemanticTypeId(0));
                        }
                        if param_ty == luna_semantic::SemanticTypeId(0) {
                            if let Decl::Param { ty: Some(ty_id), .. } = &self.arena.decls[param_id.0 as usize] {
                                param_ty = self.ctx.tables.ast_type_to_semantic.get(ty_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                            }
                        }
                        p.push(param_ty);
                    }
                }
                p
            });

            let is_async = if let Decl::Function { is_async, .. } = decl { *is_async } else { false };

            let is_extern = body.is_none();
            self.current_function = Some(Function {
                name: global_id,
                is_extern,
                is_async,
                arg_count,
                link_name,
                param_types,
                ret_ty: ret_ty_id,
                blocks: Vec::new(),
                values: Vec::new(),
            });
            self.current_block = None;

            if is_extern {
                if let Some(func) = self.current_function.take() {
                    self.module.functions.push(func);
                }
                self.current_instance = None;
                return;
            }

            self.locals.clear();
            self.lexical_scopes.clear();
            self.loop_scopes.clear();
            self.loop_break_targets.clear();
            self.loop_continue_targets.clear();
            self.push_scope(); // Function root scope
            self.next_label_id = 0;
            
            let entry_label = self.new_label("entry");
            self.start_block(entry_label);
            
            if let Decl::Function { params, .. } = decl {
                for (idx, param_id) in params.iter().enumerate() {
                    if let Decl::Param { .. } = &self.arena.decls[param_id.0 as usize] {
                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(&param_id).copied() {
                            let ty_id = self.get_symbol_type(&sym_id).unwrap_or(luna_semantic::SemanticTypeId(0));
                            let alloc_val = self.push_inst(Instruction::Alloca, ty_id);
                            self.current_function.as_mut().unwrap().values[alloc_val.0 as usize].origin = ValueOrigin::Parameter(idx as u32);
                            self.locals.insert(sym_id, alloc_val);
                            self.lexical_scopes.last_mut().unwrap().push(sym_id);
                        }
                    }
                }
            }

            if is_async {
                let future_alloca = self.push_inst(Instruction::Alloca, ret_ty_id);
                self.push_inst(Instruction::MarkInit { value: Operand::Value(future_alloca) }, ret_ty_id);
                self.current_async_future = Some(future_alloca);
                let i32_ty = luna_semantic::SemanticTypeId(3);
                let state_ptr = self.push_inst(Instruction::FieldPtr {
                    base: Operand::Value(future_alloca),
                    field_idx: 0,
                }, i32_ty);
                self.push_inst(Instruction::Store {
                    ptr: Operand::Value(state_ptr),
                    value: Operand::Number("0".to_string()),
                }, i32_ty);
            } else {
                self.current_async_future = None;
            }
            
            if let Some(body_stmt) = body {
                self.generate_fn_body(&body_stmt, ret_ty_id);
            }
            
            if let Some(func) = self.current_function.take() {
                self.module.functions.push(func);
            }
        }
        self.current_instance = None;
    }

    fn generate_fn_body(&mut self, body_stmt_id: &luna_ast::StmtId, ret_ty_id: luna_semantic::SemanticTypeId) {
        let stmt = &self.arena.stmts[body_stmt_id.0 as usize];
        match stmt {
            Stmt::Block { body, tail_expr } => {
                self.push_scope();
                for item in body {
                    self.generate_item(item);
                }
                if let Some(tail_expr_id) = tail_expr {
                    let mut returned_sym = None;
                    let expr = &self.arena.exprs[tail_expr_id.0 as usize];
                    if let Expr::Identifier { .. } = expr {
                        if let Some(sym_id) = self.ctx.tables.expr_symbols.get(tail_expr_id).copied() {
                            if self.locals.contains_key(&sym_id) {
                                returned_sym = Some(sym_id);
                            }
                        }
                    }
                    let tail_op = self.generate_expr(tail_expr_id);
                    self.pop_scope_and_drop(returned_sym);
                    if let Some(mut block) = self.current_block.take() {
                        if block.terminator.is_none() {
                            self.current_block = Some(block);
                            self.emit_drops_up_to(0, returned_sym);
                            block = self.current_block.take().unwrap();
                            let ret_val = if ret_ty_id == luna_semantic::SemanticTypeId(0) {
                                None
                            } else {
                                Some(tail_op)
                            };
                            block.terminator = Some(Terminator::Ret { value: ret_val });
                        }
                        self.current_function.as_mut().unwrap().blocks.push(block);
                    }
                    return;
                }
                self.pop_scope_and_drop(None);
            }
            Stmt::Unsafe { body } => {
                self.generate_fn_body(body, ret_ty_id);
                return;
            }
            Stmt::Expr { expr, .. } => {
                let tail_op = self.generate_expr(expr);
                if let Some(mut block) = self.current_block.take() {
                    if block.terminator.is_none() {
                        self.current_block = Some(block);
                        self.emit_drops_up_to(0, None);
                        block = self.current_block.take().unwrap();
                        let ret_val = if ret_ty_id == luna_semantic::SemanticTypeId(0) {
                            None
                        } else {
                            Some(tail_op)
                        };
                        block.terminator = Some(Terminator::Ret { value: ret_val });
                    }
                    self.current_function.as_mut().unwrap().blocks.push(block);
                }
                return;
            }
            _ => {
                self.generate_stmt(body_stmt_id);
            }
        }

        if let Some(mut block) = self.current_block.take() {
            if block.terminator.is_none() {
                self.current_block = Some(block);
                self.emit_drops_up_to(0, None);

                let ret_val = if let Some(fut_alloca) = self.current_async_future {
                    let i32_ty = luna_semantic::SemanticTypeId(3);
                    let state_ptr = self.push_inst(Instruction::FieldPtr {
                        base: Operand::Value(fut_alloca),
                        field_idx: 0,
                    }, i32_ty);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(state_ptr),
                        value: Operand::Number("-1".to_string()),
                    }, i32_ty);
                    let load_fut = self.push_inst(Instruction::Load {
                        ptr: Operand::Value(fut_alloca),
                    }, ret_ty_id);
                    Some(Operand::Value(load_fut))
                } else if ret_ty_id == luna_semantic::SemanticTypeId(0) {
                    None
                } else {
                    Some(Operand::Number("0".to_string()))
                };
                block = self.current_block.take().unwrap();
                block.terminator = Some(Terminator::Ret { value: ret_val });
            }
            self.current_function.as_mut().unwrap().blocks.push(block);
        }
    }

    fn generate_stmt(&mut self, stmt_id: &luna_ast::StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        match stmt {
            Stmt::Expr { expr, .. } => {
                self.generate_expr(expr);
            }
            Stmt::Block { body, tail_expr } => {
                self.push_scope();
                for item in body {
                    self.generate_item(item);
                }
                if let Some(expr) = tail_expr {
                    self.generate_expr(expr);
                }
                self.pop_scope_and_drop(None);
            }
            Stmt::Return { value } => {
                if let Some(fut_alloca) = self.current_async_future {
                    let i32_ty = luna_semantic::SemanticTypeId(3);
                    if let Some(expr_id) = value {
                        let ret_ty = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                        let val_op = self.generate_expr(expr_id);
                        let val_ptr = self.push_inst(Instruction::FieldPtr {
                            base: Operand::Value(fut_alloca),
                            field_idx: 1,
                        }, ret_ty);
                        self.push_inst(Instruction::Store {
                            ptr: Operand::Value(val_ptr),
                            value: val_op,
                        }, ret_ty);
                    }
                    let state_ptr = self.push_inst(Instruction::FieldPtr {
                        base: Operand::Value(fut_alloca),
                        field_idx: 0,
                    }, i32_ty);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(state_ptr),
                        value: Operand::Number("-1".to_string()),
                    }, i32_ty);
                    
                    self.emit_drops_up_to(0, None);
                    
                    let ret_ty_id = self.current_function.as_ref().unwrap().ret_ty;
                    let load_fut = self.push_inst(Instruction::Load {
                        ptr: Operand::Value(fut_alloca),
                    }, ret_ty_id);
                    self.terminate_block(Terminator::Ret { value: Some(Operand::Value(load_fut)) });
                    return;
                }

                let mut returned_sym = None;
                let val_operand = if let Some(expr_id) = value {
                    let expr = &self.arena.exprs[expr_id.0 as usize];
                    if let Expr::Identifier { .. } = expr {
                        if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id).copied() {
                            if self.locals.contains_key(&sym_id) {
                                returned_sym = Some(sym_id);
                            }
                        }
                    }
                    Some(self.generate_expr(expr_id))
                } else {
                    None
                };
                
                self.emit_drops_up_to(0, returned_sym);
                self.terminate_block(Terminator::Ret { value: val_operand });
            }
            Stmt::While { condition, body, .. } => {
                let cond_label = self.new_label("while_cond");
                let body_label = self.new_label("while_body");
                let end_label = self.new_label("while_end");

                self.terminate_block(Terminator::Br { target: cond_label.clone() });
                self.start_block(cond_label.clone());

                let cond_op = self.generate_expr(condition);
                self.terminate_block(Terminator::CondBr {
                    condition: cond_op,
                    true_target: body_label.clone(),
                    false_target: end_label.clone(),
                });

                self.start_block(body_label.clone());
                
                self.loop_scopes.push(self.lexical_scopes.len());
                self.loop_break_targets.push(end_label.clone());
                self.loop_continue_targets.push(cond_label.clone());
                
                self.generate_stmt(body);
                self.terminate_block(Terminator::Br { target: cond_label.clone() });

                self.loop_scopes.pop();
                self.loop_break_targets.pop();
                self.loop_continue_targets.pop();

                self.start_block(end_label.clone());
            }
            Stmt::Break { .. } => {
                if let (Some(&target_depth), Some(end_label)) = (self.loop_scopes.last(), self.loop_break_targets.last().cloned()) {
                    self.emit_drops_up_to(target_depth, None);
                    self.terminate_block(Terminator::Br { target: end_label });
                }
            }
            Stmt::Continue { .. } => {
                if let (Some(&target_depth), Some(cond_label)) = (self.loop_scopes.last(), self.loop_continue_targets.last().cloned()) {
                    self.emit_drops_up_to(target_depth, None);
                    self.terminate_block(Terminator::Br { target: cond_label });
                }
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let then_label = self.new_label("if_then");
                let else_label = self.new_label("if_else");
                let end_label = self.new_label("if_end");

                let cond_op = self.generate_expr(condition);
                self.terminate_block(Terminator::CondBr {
                    condition: cond_op,
                    true_target: then_label.clone(),
                    false_target: if else_branch.is_some() { else_label.clone() } else { end_label.clone() },
                });

                self.start_block(then_label.clone());
                self.generate_stmt(then_branch);
                self.terminate_block(Terminator::Br { target: end_label.clone() });

                if let Some(else_branch) = else_branch {
                    self.start_block(else_label.clone());
                    self.generate_stmt(else_branch);
                    self.terminate_block(Terminator::Br { target: end_label.clone() });
                }

                self.start_block(end_label.clone());
            }
            Stmt::Unsafe { body } => {
                self.generate_stmt(body);
            }
            Stmt::For { kind, init, cond, step, body, pattern, iterable, .. } => {
                use luna_ast::stmt::ForKind;
                match kind {
                    ForKind::CStyle => {
                        // Scope: init lives in loop's outer scope
                        self.push_scope();
                        
                        // Generate init (e.g., `dec rw i: i32 = 0`)
                        if let Some(item) = init {
                            self.generate_item(item);
                        }
                        
                        let cond_label = self.new_label("for_cond");
                        let body_label = self.new_label("for_body");
                        let step_label = self.new_label("for_step");
                        let end_label = self.new_label("for_end");
                        
                        // Jump to condition check
                        self.terminate_block(Terminator::Br { target: cond_label.clone() });
                        self.start_block(cond_label.clone());
                        
                        // Evaluate condition
                        if let Some(c) = cond {
                            let cond_op = self.generate_expr(c);
                            self.terminate_block(Terminator::CondBr {
                                condition: cond_op,
                                true_target: body_label.clone(),
                                false_target: end_label.clone(),
                            });
                        } else {
                            // No condition → infinite loop (always true)
                            self.terminate_block(Terminator::Br { target: body_label.clone() });
                        }
                        
                        // Body
                        self.start_block(body_label.clone());
                        
                        // Push loop targets: break→end, continue→step
                        self.loop_scopes.push(self.lexical_scopes.len());
                        self.loop_break_targets.push(end_label.clone());
                        self.loop_continue_targets.push(step_label.clone());
                        
                        self.generate_stmt(body);
                        
                        // Fall through to step
                        self.terminate_block(Terminator::Br { target: step_label.clone() });
                        
                        // Step
                        self.start_block(step_label.clone());
                        if let Some(s) = step {
                            self.generate_expr(s);
                        }
                        self.terminate_block(Terminator::Br { target: cond_label.clone() });
                        
                        // Pop loop targets
                        self.loop_scopes.pop();
                        self.loop_break_targets.pop();
                        self.loop_continue_targets.pop();
                        
                        // End block + drop init scope variables
                        self.start_block(end_label.clone());
                        self.pop_scope_and_drop(None);
                    }
                    ForKind::ForEach => {
                        let pat = pattern.as_ref().unwrap();
                        let iter_expr = iterable.as_ref().unwrap();
                        let iter = &self.arena.exprs[iter_expr.0 as usize];
                        
                        if let luna_ast::Expr::Binary { op: luna_ast::expr::BinaryOp::Range, left, right, .. } = iter {
                            let left_op = self.generate_expr(left);
                            let right_op = self.generate_expr(right);
                            let left_ty_id = self.ctx.tables.expr_types.get(left).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                            
                            let iter_cur = self.push_inst(Instruction::Alloca, left_ty_id);
                            self.push_inst(Instruction::Store { ptr: Operand::Value(iter_cur), value: left_op }, left_ty_id);
                            
                            let cond_label = self.new_label("for_cond");
                            let body_label = self.new_label("for_body");
                            let step_label = self.new_label("for_step");
                            let end_label = self.new_label("for_end");
                            
                            self.terminate_block(Terminator::Br { target: cond_label.clone() });
                            self.start_block(cond_label.clone());
                            
                            let cur_val = self.push_inst(Instruction::Load { ptr: Operand::Value(iter_cur) }, left_ty_id);
                            let cond_val = self.push_inst(Instruction::LessThan { left: Operand::Value(cur_val), right: right_op.clone() }, self.ctx.types.bool_id());
                            self.terminate_block(Terminator::CondBr {
                                condition: Operand::Value(cond_val),
                                true_target: body_label.clone(),
                                false_target: end_label.clone(),
                            });
                            
                            self.start_block(body_label.clone());
                            self.push_scope();
                            self.loop_scopes.push(self.lexical_scopes.len());
                            self.loop_break_targets.push(end_label.clone());
                            self.loop_continue_targets.push(step_label.clone());
                            
                            if let Some(pat_sym) = self.ctx.tables.pat_symbols.get(pat) {
                                let pat_ty_id = self.get_symbol_type(pat_sym).unwrap_or(left_ty_id);
                                let pat_var = self.push_inst(Instruction::Alloca, pat_ty_id);
                                self.locals.insert(*pat_sym, pat_var);
                                self.push_inst(Instruction::Store { ptr: Operand::Value(pat_var), value: Operand::Value(cur_val) }, pat_ty_id);
                                if let Some(scope) = self.lexical_scopes.last_mut() {
                                    scope.push(*pat_sym);
                                }
                            }
                            
                            self.generate_stmt(body);
                            
                            self.terminate_block(Terminator::Br { target: step_label.clone() });
                            
                            self.start_block(step_label.clone());
                            let cur_val_for_add = self.push_inst(Instruction::Load { ptr: Operand::Value(iter_cur) }, left_ty_id);
                            let inc_val = self.push_inst(Instruction::Add { left: Operand::Value(cur_val_for_add), right: Operand::Number("1".to_string()) }, left_ty_id);
                            self.push_inst(Instruction::Store { ptr: Operand::Value(iter_cur), value: Operand::Value(inc_val) }, left_ty_id);
                            self.terminate_block(Terminator::Br { target: cond_label.clone() });
                            
                            self.loop_scopes.pop();
                            self.loop_break_targets.pop();
                            self.loop_continue_targets.pop();
                            
                            self.start_block(end_label.clone());
                            self.pop_scope_and_drop(None);
                        } else {
                            self.diagnostics.push(luna_common::Diagnostic::error("for-each over non-range iterables is not yet supported".to_string()));
                        }
                    }
                }
            }
            Stmt::Comptime { .. } => {
                unreachable!("ICE: Stmt::Comptime reached MVIR generator, should have been blocked by Typechecker");
            }
            _ => unreachable!("ICE: Unhandled variant, should be impossible after semantic invariants")
        }
    }

    fn generate_block_expr(&mut self, stmt_id: &luna_ast::StmtId) -> Operand {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        if let Stmt::Block { body, tail_expr } = stmt {
            let mut last_ret = None;
            for (i, item) in body.iter().enumerate() {
                if i == body.len() - 1 && tail_expr.is_none() {
                    if let Item::Stmt(s) = item {
                        last_ret = Some(self.generate_block_expr(s));
                        continue;
                    }
                }
                self.generate_item(item);
            }
            if let Some(expr) = tail_expr {
                return self.generate_expr(expr);
            }
            if let Some(ret) = last_ret {
                return ret;
            }
        } else if let Stmt::Expr { expr, .. } = stmt {
            return self.generate_expr(expr);
        } else if let Stmt::Unsafe { body } = stmt {
            return self.generate_block_expr(body);
        } else {
            self.generate_stmt(stmt_id);
        }
        Operand::Number("0".to_string())
    }

    fn get_expr_span(&self, expr: &luna_ast::Expr) -> Option<luna_common::Span> {
        use luna_ast::Expr;
        match expr {
            Expr::Literal(tok, _) => Some(tok.span),
            Expr::Identifier { segments, .. } => segments.first().copied(),
            Expr::Member { member, .. } => Some(*member),
            Expr::Call { callee, .. } => self.get_expr_span(&self.arena.exprs[callee.0 as usize]),
            Expr::MethodCall { method_name, .. } => Some(*method_name),
            Expr::StructInit { path, .. } => path.first().copied(),
            Expr::Match { match_span, .. } => Some(*match_span),
            Expr::Try { try_span, .. } => Some(*try_span),
            Expr::Lambda { .. } => None,
            Expr::Assign { lvalue, .. } => self.get_expr_span(&self.arena.exprs[lvalue.0 as usize]),
            Expr::Binary { left, .. } => self.get_expr_span(&self.arena.exprs[left.0 as usize]),
            Expr::Unary { operand, .. } => self.get_expr_span(&self.arena.exprs[operand.0 as usize]),
            Expr::Cast { expr, .. } => self.get_expr_span(&self.arena.exprs[expr.0 as usize]),
            Expr::Index { base, .. } => self.get_expr_span(&self.arena.exprs[base.0 as usize]),
            Expr::TupleIndex { object, .. } => self.get_expr_span(&self.arena.exprs[object.0 as usize]),
            Expr::Await { expr } => self.get_expr_span(&self.arena.exprs[expr.0 as usize]),
            Expr::Sizeof { .. } | Expr::Alignof { .. } | Expr::Comptime { .. } => None,
            Expr::MacroCall { span, .. } => Some(*span),
            Expr::ArrayLiteral { elements } | Expr::TupleLiteral { elements } => {
                elements.first().and_then(|e| self.get_expr_span(&self.arena.exprs[e.0 as usize]))
            }
        }
    }

    fn generate_expr(&mut self, expr_id: &luna_ast::ExprId) -> Operand {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        let span = self.get_expr_span(expr);
        let prev_span = self.current_span.clone();
        if span.is_some() {
            self.current_span = span;
        }

        let mut result = self.generate_expr_inner(expr_id);
        
        if let Some(coercion) = self.ctx.tables.coercions.get(expr_id) {
            match coercion {
                luna_semantic::CoercionKind::ConcreteToDyn { trait_sym, concrete_sym } => {
                    let trait_name = self.ctx.symbol_table.get_symbol(*trait_sym).name.clone();
                    let concrete_name = self.ctx.symbol_table.get_symbol(*concrete_sym).name.clone();
                    let vtable_global = GlobalId {
                        name: format!("vtable_{}_{}", trait_name, concrete_name),
                        symbol_id: Some(*trait_sym),
                    };
                    let ty_id = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                    let trait_obj_val = self.push_inst(Instruction::MakeTraitObject {
                        data_ptr: result,
                        vtable: vtable_global,
                        trait_sym: *trait_sym,
                        concrete_sym: *concrete_sym,
                    }, ty_id);
                    result = Operand::Value(trait_obj_val);
                }
                luna_semantic::CoercionKind::ArrayToSlice { element_type: _, length } => {
                    let ty_id = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                    let len_operand = Operand::Number(length.to_string());
                    let slice_val = self.push_inst(Instruction::MakeSlice {
                        data_ptr: result,
                        len: len_operand,
                    }, ty_id);
                    result = Operand::Value(slice_val);
                }
                luna_semantic::CoercionKind::RefMutToShared => {
                    // Representation is unchanged
                }
            }
        }
        
        let ty_id = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
        let ty = self.ctx.types.get(ty_id);
        if let luna_semantic::SemanticType::Never = ty {
            self.terminate_block(Terminator::Unreachable);
        }
        
        self.current_span = prev_span;
        result
    }

    fn generate_expr_inner(&mut self, expr_id: &luna_ast::ExprId) -> Operand {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        let mut ty_id = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
        if let Some(inst_ptr) = self.current_instance {
            let inst = unsafe { &*inst_ptr };
            if let Some(&mono_ty) = inst.expr_types.get(expr_id) {
                ty_id = mono_ty;
            }
        }
        
        if let Some(ct_val) = self.ctx.comptime_values.get(expr_id).cloned() {
            return self.materialize_comptime_value(&ct_val, ty_id);
        }
        
        match expr {
            Expr::Literal(tok, text) => {
                match tok.kind {
                    luna_lexer::TokenKind::IntegerLiteral | luna_lexer::TokenKind::FloatLiteral => {
                        
                        Operand::Number(text.clone())
                    }
                    luna_lexer::TokenKind::StringLiteral => {
                        
                        Operand::StringRef(text.clone())
                    }
                    luna_lexer::TokenKind::CharLiteral => {
                        
                        Operand::Char(text.clone())
                    }
                    luna_lexer::TokenKind::KwTrue => Operand::Boolean(true),
                    luna_lexer::TokenKind::KwFalse => Operand::Boolean(false),
                    _ => Operand::Number("null".to_string()),
                }
            }
            Expr::Binary { op, left, right, .. } => {
                if matches!(op, luna_ast::expr::BinaryOp::LogicAnd | luna_ast::expr::BinaryOp::LogicOr) {
                    let left_op = self.generate_expr(left);
                    let result_alloca = self.push_inst(Instruction::Alloca, ty_id);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(result_alloca),
                        value: left_op.clone(),
                    }, ty_id);
                    
                    let right_label = self.new_label("logical_right");
                    let end_label = self.new_label("logical_end");
                    
                    if *op == luna_ast::expr::BinaryOp::LogicAnd {
                        self.terminate_block(Terminator::CondBr {
                            condition: left_op,
                            true_target: right_label.clone(),
                            false_target: end_label.clone(),
                        });
                    } else {
                        self.terminate_block(Terminator::CondBr {
                            condition: left_op,
                            true_target: end_label.clone(),
                            false_target: right_label.clone(),
                        });
                    }
                    
                    self.start_block(right_label);
                    let right_op = self.generate_expr(right);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(result_alloca),
                        value: right_op,
                    }, ty_id);
                    self.terminate_block(Terminator::Br { target: end_label.clone() });
                    
                    self.start_block(end_label);
                    let load_val = self.push_inst(Instruction::Load {
                        ptr: Operand::Value(result_alloca),
                    }, ty_id);
                    Operand::Value(load_val)
                } else {
                    let left_op = self.generate_expr(left);
                    let right_op = self.generate_expr(right);
                    let mut bin_ty = ty_id;
                    if bin_ty == luna_semantic::SemanticTypeId(0) {
                        match op {
                            luna_ast::expr::BinaryOp::Eq | luna_ast::expr::BinaryOp::Ne |
                            luna_ast::expr::BinaryOp::Lt | luna_ast::expr::BinaryOp::Le |
                            luna_ast::expr::BinaryOp::Gt | luna_ast::expr::BinaryOp::Ge => {
                                bin_ty = self.ctx.types.bool_id();
                            }
                            _ => {
                                if let Operand::Value(v) = &left_op {
                                    bin_ty = self.current_function.as_ref().unwrap().values[v.0 as usize].ty;
                                } else if let Operand::Value(v) = &right_op {
                                    bin_ty = self.current_function.as_ref().unwrap().values[v.0 as usize].ty;
                                }
                            }
                        }
                    }
                    let inst = match op {
                        luna_ast::expr::BinaryOp::Add => Instruction::Add { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Sub => Instruction::Sub { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Mul => Instruction::Mul { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Div => Instruction::Div { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Mod => Instruction::Rem { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Eq => Instruction::Eq { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Ne => Instruction::NotEq { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Lt => Instruction::LessThan { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Le => Instruction::LessOrEq { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Gt => Instruction::GreaterThan { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::Ge => Instruction::GreaterOrEq { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::BitAnd => Instruction::BitAnd { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::BitOr => Instruction::BitOr { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::BitXor => Instruction::BitXor { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::LShift => Instruction::Shl { left: left_op, right: right_op },
                        luna_ast::expr::BinaryOp::RShift => Instruction::Shr { left: left_op, right: right_op },
                        _ => panic!("Unsupported binary operator in MVIR generation: {:?}", op),
                    };
                    let val_id = self.push_inst(inst, bin_ty);
                    Operand::Value(val_id)
                }
            }
            Expr::Call { callee, args, .. } => {
                if let Some(&method_idx) = self.ctx.tables.dyn_method_indices.get(callee) {
                    let callee_expr = &self.arena.exprs[callee.0 as usize];
                    if let luna_ast::Expr::Member { object, .. } = callee_expr {
                        let obj_op = self.generate_expr(object);
                        let mut arg_ops = Vec::new();
                        for arg in args {
                            arg_ops.push(self.generate_expr(&arg.value));
                        }
                        let call_val = self.push_inst(Instruction::CallVirt {
                            obj: obj_op,
                            method_idx,
                            args: arg_ops,
                        }, ty_id);
                        return Operand::Value(call_val);
                    }
                }

                // Check if callee is a struct method call: obj.method(args...)
                let callee_expr = &self.arena.exprs[callee.0 as usize];
                if let luna_ast::Expr::Member { object, .. } = callee_expr {
                    if let Some(&m_sym) = self.ctx.tables.expr_symbols.get(callee) {
                        let mut m_name = self.ctx.symbol_table.get_symbol(m_sym).name.clone();
                        let is_mono = if let Some(inst_ptr) = self.current_instance {
                            let inst = unsafe { &*inst_ptr };
                            inst.mono_calls.get(expr_id).map_or(false, |m| !m.subst.is_empty())
                        } else {
                            self.ctx.instantiated_functions.iter().any(|f| f.mono_calls.get(expr_id).map_or(false, |m| !m.subst.is_empty()))
                        };
                        if is_mono && !m_name.ends_with("_mono") {
                            m_name = format!("{}_mono", m_name);
                        }
                        let obj_op = self.generate_expr(object);
                        let mut arg_ops = vec![obj_op];
                        for arg in args {
                            arg_ops.push(self.generate_expr(&arg.value));
                        }
                        let call_val = self.push_inst(Instruction::CallDirect {
                            callee: GlobalId {
                                name: m_name,
                                symbol_id: Some(m_sym),
                            },
                            args: arg_ops,
                        }, ty_id);
                        return Operand::Value(call_val);
                    }
                }
                
                let callee_op = self.generate_expr(callee);
                let mut arg_ops = Vec::new();
                for arg in args {
                    arg_ops.push(self.generate_expr(&arg.value));
                }
                
                // Check if callee is an enum variant
                let is_variant = if let Some(sym_id) = self.ctx.tables.expr_symbols.get(callee) {
                    let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                    if let luna_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                        Some((idx, symbol.decl_id.unwrap()))
                    } else { None }
                } else { None };

                if let Some((variant_idx, decl_id)) = is_variant {
                    let mut enum_ty = ty_id;
                    if enum_ty == luna_semantic::SemanticTypeId(0) {
                        if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {
                            if let Some(ety) = self.get_symbol_type(enum_sym_id) {
                                enum_ty = ety;
                            }
                        }
                    }
                    if enum_ty == luna_semantic::SemanticTypeId(0) {
                        if let Some(func) = &self.current_function {
                            if matches!(self.ctx.types.get(func.ret_ty), luna_semantic::SemanticType::Enum(..)) {
                                enum_ty = func.ret_ty;
                            }
                        }
                    }
                    let call_val = self.push_inst(Instruction::Variant {
                        enum_ty,
                        variant_idx,
                        args: arg_ops,
                    }, enum_ty);
                    return Operand::Value(call_val);
                } else {
                    let callee_ty_id = self.ctx.tables.expr_types.get(callee).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                    let is_closure = matches!(self.ctx.types.get(callee_ty_id), luna_semantic::SemanticType::Closure(..));
                    
                    let call_val = self.push_inst(if is_closure {
                        Instruction::CallClosure { closure: callee_op, args: arg_ops }
                    } else {
                        match callee_op {
                            Operand::Global(mut id) => {
                                let is_mono = if let Some(inst_ptr) = self.current_instance {
                                    let inst = unsafe { &*inst_ptr };
                                    inst.mono_calls.get(expr_id).map_or(false, |m| !m.subst.is_empty())
                                } else {
                                    self.ctx.instantiated_functions.iter().any(|f| f.mono_calls.get(expr_id).map_or(false, |m| !m.subst.is_empty()))
                                };
                                if is_mono && !id.name.ends_with("_mono") {
                                    id.name = format!("{}_mono", id.name);
                                }
                                Instruction::CallDirect { callee: id, args: arg_ops }
                            }
                            _ => Instruction::CallIndirect { callee: callee_op, args: arg_ops },
                        }
                    }, ty_id);
                    Operand::Value(call_val)
                }
            }
            Expr::Identifier { segments, .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id).copied() {
                    if let Some(&val_id) = self.locals.get(&sym_id) {
                        // Let's check if the local is a pointer. Wait, locals is a map to Alloca.
                        // Wait, what if sym_id is EnumVariant? Locals won't have it.
                    }
                    
                    let symbol = self.ctx.symbol_table.get_symbol(sym_id);
                    if let luna_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                        if let Some(decl_id) = symbol.decl_id {
                            if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {
                                if let Some(enum_ty) = self.get_symbol_type(enum_sym_id) {
                                    let variant_val = self.push_inst(Instruction::Variant {
                                        enum_ty,
                                        variant_idx: idx,
                                        args: Vec::new(),
                                    }, ty_id);
                                    return Operand::Value(variant_val);
                                }
                            }
                        }
                    }
                    
                    if let Some(ct_val) = self.ctx.const_values.get(&sym_id).cloned() {
                        return self.materialize_comptime_value(&ct_val, ty_id);
                    }

                    if let Some(&val_id) = self.locals.get(&sym_id) {
                        let mut sym_ty = if ty_id != luna_semantic::SemanticTypeId(0) {
                            ty_id
                        } else {
                            self.get_symbol_type(&sym_id).unwrap_or(ty_id)
                        };
                        if sym_ty == luna_semantic::SemanticTypeId(0) {
                            sym_ty = self.current_function.as_ref().unwrap().values[val_id.0 as usize].ty;
                        }
                        let load_val = self.push_inst(Instruction::Load {
                            ptr: Operand::Value(val_id),
                        }, sym_ty);
                        return Operand::Value(load_val);
                    }
                    
                    let sym_name = if (sym_id.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        self.ctx.symbol_table.symbols[sym_id.0 as usize].name.clone()
                    } else {
                        format!("global_{}", segments[0].start)
                    };
                    return Operand::Global(GlobalId {
                        name: sym_name,
                        symbol_id: Some(sym_id),
                    });
                }
                Operand::Number("0".to_string())
            }
            Expr::Assign { op: _, lvalue, value } => {
                let ptr_op = self.generate_lvalue(lvalue);
                let val_op = self.generate_expr(value);
                let val_ty_id = self.ctx.tables.expr_types.get(value).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                self.push_inst_span(Instruction::Store {
                    ptr: ptr_op,
                    value: val_op,
                }, val_ty_id, self.extract_expr_span(expr_id));
                Operand::Number("0".to_string())
            }
            Expr::StructInit { fields, .. } => {
                let struct_alloca = self.push_inst(Instruction::Alloca, ty_id);
                // We use MarkInit if empty struct so it's considered initialized
                if fields.is_empty() {
                    self.push_inst(Instruction::MarkInit { value: Operand::Value(struct_alloca) }, ty_id);
                } else {
                    let indices_opt = self.ctx.tables.expr_struct_init_indices.get(expr_id).cloned();
                    let computed_indices: Vec<u32> = if let Some(indices) = indices_opt {
                        indices
                    } else {
                        let mut resolved_indices = Vec::with_capacity(fields.len());
                        let declared_fields_opt = match self.ctx.types.get(ty_id) {
                            luna_semantic::SemanticType::Struct(sym_id, _, _) => {
                                self.ctx.symbol_table.get_symbol(*sym_id).decl_id
                                    .and_then(|did| {
                                        if (did.0 as usize) < self.arena.decls.len() {
                                            if let Decl::Struct { fields: df, .. } = &self.arena.decls[did.0 as usize] {
                                                Some(df.clone())
                                            } else { None }
                                        } else { None }
                                    })
                            }
                            _ => None,
                        };
                        for (i, field) in fields.iter().enumerate() {
                            let idx = if let Some(ref df) = declared_fields_opt {
                                let f_name = self.get_span_text(field.name);
                                df.iter().position(|d| self.get_span_text(d.name) == f_name)
                                    .map(|pos| pos as u32)
                                    .unwrap_or(i as u32)
                            } else {
                                i as u32
                            };
                            resolved_indices.push(idx);
                        }
                        resolved_indices
                    };

                    for (i, field) in fields.iter().enumerate() {
                        let field_idx = computed_indices[i];
                        if field_idx != u32::MAX {
                            let val_op = self.generate_expr(&field.value);
                            // Ensure proper field ptr type
                            let ptr = self.push_inst(Instruction::FieldPtr {
                                base: Operand::Value(struct_alloca),
                                field_idx,
                            }, ty_id);
                            
                            self.push_inst(Instruction::Store {
                                ptr: Operand::Value(ptr),
                                value: val_op,
                            }, self.ctx.types.bool_id());
                        }
                    }
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(struct_alloca),
                }, ty_id);
                Operand::Value(load_val)
            }
            Expr::TupleLiteral { elements } => {
                let tuple_alloca = self.push_inst(Instruction::Alloca, ty_id);
                if elements.is_empty() {
                    self.push_inst(Instruction::MarkInit { value: Operand::Value(tuple_alloca) }, ty_id);
                } else {
                    for (i, elem) in elements.iter().enumerate() {
                        let val_op = self.generate_expr(elem);
                        let ptr = self.push_inst(Instruction::FieldPtr {
                            base: Operand::Value(tuple_alloca),
                            field_idx: i as u32,
                        }, ty_id);
                        
                        self.push_inst(Instruction::Store {
                            ptr: Operand::Value(ptr),
                            value: val_op,
                        }, self.ctx.types.bool_id());
                    }
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(tuple_alloca),
                }, ty_id);
                Operand::Value(load_val)
            }
            Expr::ArrayLiteral { elements } => {
                let array_alloca = self.push_inst(Instruction::Alloca, ty_id);
                if elements.is_empty() {
                    self.push_inst(Instruction::MarkInit { value: Operand::Value(array_alloca) }, ty_id);
                } else {
                    for (i, elem) in elements.iter().enumerate() {
                        let val_op = self.generate_expr(elem);
                        let ptr = self.push_inst(Instruction::FieldPtr {
                            base: Operand::Value(array_alloca),
                            field_idx: i as u32,
                        }, ty_id);
                        
                        self.push_inst(Instruction::Store {
                            ptr: Operand::Value(ptr),
                            value: val_op,
                        }, self.ctx.types.bool_id());
                    }
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(array_alloca),
                }, ty_id);
                Operand::Value(load_val)
            }
            Expr::Member { .. } | Expr::TupleIndex { .. } => {
                let ptr_op = self.generate_lvalue(expr_id);
                let mut load_ty = ty_id;
                if load_ty == luna_semantic::SemanticTypeId(0) {
                    if let Operand::Value(v) = ptr_op {
                        let ptr_ty = self.current_function.as_ref().unwrap().values[v.0 as usize].ty;
                        load_ty = ptr_ty;
                    }
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: ptr_op,
                }, load_ty);
                Operand::Value(load_val)
            }
            Expr::MethodCall { object, args, method_name, .. } => {
                if let Some(&method_idx) = self.ctx.tables.dyn_method_indices.get(expr_id) {
                    let obj_op = self.generate_expr(object);
                    let mut arg_ops = Vec::new();
                    for arg in args {
                        arg_ops.push(self.generate_expr(&arg.value));
                    }
                    let call_val = self.push_inst(Instruction::CallVirt {
                        obj: obj_op,
                        method_idx,
                        args: arg_ops,
                    }, ty_id);
                    return Operand::Value(call_val);
                }

                if let Some(&m_sym) = self.ctx.tables.expr_symbols.get(expr_id) {
                    let mut m_name = self.ctx.symbol_table.get_symbol(m_sym).name.clone();
                    let is_mono = if let Some(inst_ptr) = self.current_instance {
                        let inst = unsafe { &*inst_ptr };
                        inst.mono_calls.get(expr_id).map_or(false, |m| !m.subst.is_empty())
                    } else {
                        self.ctx.instantiated_functions.iter().any(|f| f.mono_calls.get(expr_id).map_or(false, |m| !m.subst.is_empty()))
                    };
                    if is_mono && !m_name.ends_with("_mono") {
                        m_name = format!("{}_mono", m_name);
                    }
                    let m_ty_opt = self.get_symbol_type(&m_sym);
                    let (is_ref_self, is_rw_self) = if let Some(m_ty) = m_ty_opt {
                        if let luna_semantic::SemanticType::Function { params, .. } = self.ctx.types.get(m_ty) {
                            if let Some(&first_param) = params.first() {
                                match self.ctx.types.get(first_param) {
                                    luna_semantic::SemanticType::Reference(_, mutability, _) => (
                                        true,
                                        matches!(mutability, luna_semantic::ty::Mutability::Mutable),
                                    ),
                                    _ => (false, false),
                                }
                            } else { (false, false) }
                        } else { (false, false) }
                    } else { (false, false) };

                    let is_already_ref = matches!(
                        self.ctx.types.get(
                            self.ctx.tables.expr_types.get(object).copied().unwrap_or(luna_semantic::SemanticTypeId(0))
                        ),
                        luna_semantic::SemanticType::Reference(..) | luna_semantic::SemanticType::Pointer(..)
                    );

                    let obj_op = if is_ref_self && !is_already_ref {
                        let lval = self.generate_lvalue(object);
                        let borrow_val = self.push_inst(Instruction::Borrow { is_rw: is_rw_self, base: lval }, ty_id);
                        Operand::Value(borrow_val)
                    } else {
                        self.generate_expr(object)
                    };

                    let mut arg_ops = vec![obj_op];
                    for arg in args {
                        arg_ops.push(self.generate_expr(&arg.value));
                    }
                    let call_val = self.push_inst(Instruction::CallDirect {
                        callee: GlobalId {
                            name: m_name,
                            symbol_id: Some(m_sym),
                        },
                        args: arg_ops,
                    }, ty_id);
                    return Operand::Value(call_val);
                }
                Operand::Number("0".to_string())
            }
            Expr::Index { base, index } => {
                let base_op = self.generate_expr(base);
                let index_op = self.generate_expr(index);
                
                // Get base type to extract array length
                let base_ty_id = self.ctx.tables.expr_types.get(base).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                let base_ty = self.ctx.types.get(base_ty_id).clone();
                
                if let luna_semantic::SemanticType::Array(_, len) = base_ty {
                    let len_op = Operand::Number(len.to_string());
                    self.push_inst(Instruction::BoundsCheck {
                        index: index_op.clone(),
                        len: len_op,
                    }, self.ctx.types.bool_id());
                }
                
                // Emitting the actual element access logic will be done by pointer offset later.
                // For now we just return an invalid operand or 0 since full code-gen for elements isn't done.
                Operand::Number("0".to_string())
            }
            Expr::Match { subject, arms, match_span: _ } => {
                let subject_op = self.generate_expr(subject);
                let match_ty_id = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                
                let result_alloca = self.push_inst(Instruction::Alloca, match_ty_id);
                let end_label = self.new_label("match_end");
                
                let mut next_arm_label = self.new_label("match_arm");
                
                for (i, arm) in arms.iter().enumerate() {
                    self.terminate_block(Terminator::Br { target: next_arm_label.clone() });
                    self.start_block(next_arm_label.clone());
                    
                    next_arm_label = if i == arms.len() - 1 {
                        self.new_label("match_unreachable") // We assume exhaustion
                    } else {
                        self.new_label("match_arm")
                    };
                    
                    let is_match = self.generate_pat_match(&arm.pattern, &subject_op);
                    
                    let body_label = self.new_label("match_body");
                    self.terminate_block(Terminator::CondBr {
                        condition: is_match,
                        true_target: body_label.clone(),
                        false_target: next_arm_label.clone(),
                    });
                    
                    self.start_block(body_label);
                    
                    self.bind_pat_vars(&arm.pattern, &subject_op);
                    let body_op = self.generate_block_expr(&arm.body);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(result_alloca),
                        value: body_op,
                    }, match_ty_id);
                    
                    self.terminate_block(Terminator::Br { target: end_label.clone() });
                }
                
                self.start_block(next_arm_label);
                self.terminate_block(Terminator::Unreachable);
                
                self.start_block(end_label);
                
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(result_alloca),
                }, match_ty_id);
                
                Operand::Value(load_val)
            }

            Expr::Lambda { .. } => {
                let env_ty_id = self.ctx.tables.closure_env_types.get(expr_id).copied().unwrap_or_else(|| panic!("ICE: Missing closure_env_types for closure ID {:?}", expr_id));
                let env_ptr = self.push_inst(Instruction::HeapAlloc, env_ty_id);
                
                let mut captures_info = Vec::new();
                if let Some(bindings) = self.ctx.tables.closure_capture_bindings.get(expr_id) {
                    for binding in bindings {
                        let local_val = self.locals.get(&binding.symbol).copied().unwrap_or(crate::ValueId(0));
                        captures_info.push(crate::CaptureInfo {
                            symbol: binding.symbol,
                            mode: binding.mode,
                            source: local_val,
                            env_field: binding.env_field,
                            ty: binding.ty,
                            env_ty: binding.env_ty,
                        });
                    }
                }
                
                let mut fn_name = format!("closure_{}", expr_id.0);
                if let Some(instance) = &self.current_function {
                    if instance.name.name.ends_with("_mono") {
                        fn_name = format!("{}_mono", fn_name);
                    }
                }
                let func_id = GlobalId {
                    name: fn_name,
                    symbol_id: None,
                };
                
                let make_closure_val = self.push_inst(Instruction::MakeClosure {
                    func: func_id,
                    env_ptr: Operand::Value(env_ptr),
                    captures: captures_info,
                }, ty_id);
                Operand::Value(make_closure_val)
            }
            Expr::Await { expr } => {
                let fut_op = self.generate_expr(expr);
                let out_ty = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                
                let await_val = self.push_inst(Instruction::Await { future: fut_op }, out_ty);
                Operand::Value(await_val)
            }
            Expr::Try { expr: inner, .. } => {
                let inner_op = self.generate_expr(inner);
                let inner_ty_id = self.ctx.tables.expr_types.get(inner).copied().unwrap_or(luna_semantic::SemanticTypeId(0));
                
                // 1. Get the branch method
                let branch_decl = self.ctx.tables.try_branch_methods.get(expr_id).copied().unwrap();
                let branch_ret_ty = if let Some(luna_semantic::SemanticType::Function { return_type, .. }) = self.ctx.tables.symbol_types.get(&self.ctx.tables.decl_symbols.get(&branch_decl).copied().unwrap()).map(|&t| self.ctx.types.get(t).clone()) {
                    return_type
                } else {
                    luna_semantic::SemanticTypeId(0)
                };
                
                // Call `branch`
                let flow_val = self.push_inst(Instruction::CallDirect {
                    callee: GlobalId {
                        name: self.ctx.symbol_table.get_symbol(self.ctx.tables.decl_symbols.get(&branch_decl).copied().unwrap()).name.clone(),
                        symbol_id: Some(self.ctx.tables.decl_symbols.get(&branch_decl).copied().unwrap()),
                    },
                    args: vec![inner_op.clone()],
                    
                }, branch_ret_ty);
                
                // Get ControlFlow's Break and Continue variants from LangItem!
                let break_sym = self.ctx.lang_items.get(luna_semantic::lang_item::LangItem::ControlFlowBreak).unwrap();
                let continue_sym = self.ctx.lang_items.get(luna_semantic::lang_item::LangItem::ControlFlowContinue).unwrap();
                
                let break_idx = match self.ctx.symbol_table.get_symbol(break_sym).kind {
                    luna_semantic::symbol::SymbolKind::EnumVariant(idx) => idx,
                    _ => panic!("Expected EnumVariant for ControlFlowBreak"),
                };
                let continue_idx = match self.ctx.symbol_table.get_symbol(continue_sym).kind {
                    luna_semantic::symbol::SymbolKind::EnumVariant(idx) => idx,
                    _ => panic!("Expected EnumVariant for ControlFlowContinue"),
                };
                
                let success_label = self.new_label("try_success");
                let failure_label = self.new_label("try_failure");
                
                // Tag of the flow_val
                let tag_val = self.push_inst(Instruction::Tag {
                    value: Operand::Value(flow_val),
                }, self.ctx.types.bool_id());
                
                let is_failure = self.push_inst(Instruction::Eq {
                    left: Operand::Value(tag_val),
                    right: Operand::Number(break_idx.to_string()),
                }, self.ctx.types.bool_id());
                
                self.terminate_block(Terminator::CondBr {
                    condition: Operand::Value(is_failure),
                    true_target: failure_label.clone(),
                    false_target: success_label.clone(),
                });
                
                // -- Failure Path --
                self.start_block(failure_label);
                
                let flow_sem_ty = self.ctx.types.get(branch_ret_ty).clone();
                let residual_ty = if let luna_semantic::SemanticType::Enum(_, _, variant_tys) = &flow_sem_ty {
                    variant_tys[break_idx as usize]
                } else {
                    luna_semantic::SemanticTypeId(0)
                };
                
                let residual_val = self.push_inst(Instruction::Extract {
                    value: Operand::Value(flow_val),
                    variant_idx: break_idx,
                    field_idx: 0,
                }, residual_ty);
                
                // Call `FromResidual::from_residual`
                let from_residual_decl = self.ctx.tables.try_from_residual_methods.get(expr_id).copied().unwrap();
                let expected_ret_ty = self.current_function.as_ref().unwrap().ret_ty;
                
                let ret_val = self.push_inst(Instruction::CallDirect {
                    callee: GlobalId {
                        name: self.ctx.symbol_table.get_symbol(self.ctx.tables.decl_symbols.get(&from_residual_decl).copied().unwrap()).name.clone(),
                        symbol_id: Some(self.ctx.tables.decl_symbols.get(&from_residual_decl).copied().unwrap()),
                    },
                    args: vec![Operand::Value(residual_val)],
                    
                }, expected_ret_ty);
                
                self.emit_drops_up_to(0, None);
                self.terminate_block(Terminator::Ret { value: Some(Operand::Value(ret_val)) });
                
                // -- Success Path --
                self.start_block(success_label);
                
                let output_ty = if let luna_semantic::SemanticType::Enum(_, _, variant_tys) = &flow_sem_ty {
                    variant_tys[continue_idx as usize]
                } else {
                    luna_semantic::SemanticTypeId(0)
                };
                
                let output_val = self.push_inst(Instruction::Extract {
                    value: Operand::Value(flow_val),
                    variant_idx: continue_idx,
                    field_idx: 0,
                }, output_ty);
                
                Operand::Value(output_val)
            }
            Expr::Unary { op, operand } => {
                use luna_ast::expr::UnaryOp;
                match op {
                    UnaryOp::Ref => {
                        let lval = self.generate_lvalue(operand);
                        let val = self.push_inst(Instruction::Borrow { is_rw: false, base: lval }, ty_id);
                        Operand::Value(val)
                    }
                    UnaryOp::RefMut => {
                        let lval = self.generate_lvalue(operand);
                        let val = self.push_inst(Instruction::Borrow { is_rw: true, base: lval }, ty_id);
                        Operand::Value(val)
                    }
                    UnaryOp::Deref => {
                        let ptr_op = self.generate_expr(operand);
                        let mut load_ty = ty_id;
                        if load_ty == luna_semantic::SemanticTypeId(0) {
                            if let Operand::Value(v) = &ptr_op {
                                let ptr_ty = self.current_function.as_ref().unwrap().values[v.0 as usize].ty;
                                match self.ctx.types.get(ptr_ty) {
                                    luna_semantic::SemanticType::Pointer(_, inner) |
                                    luna_semantic::SemanticType::Reference(_, _, inner) => {
                                        load_ty = *inner;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        let val = self.push_inst(Instruction::Load { ptr: ptr_op }, load_ty);
                        Operand::Value(val)
                    }
                    UnaryOp::Neg => {
                        let val_op = self.generate_expr(operand);
                        let val = self.push_inst(Instruction::Sub { left: Operand::Number("0".to_string()), right: val_op }, ty_id);
                        Operand::Value(val)
                    }
                    _ => self.generate_expr(operand),
                }
            }
            Expr::Sizeof { target_type } => {
                let target_ty_id = self.resolve_ast_type(target_type);
                let usize_ty = self.ctx.types.usize_id();
                let res_ty = if ty_id != luna_semantic::SemanticTypeId(0) { ty_id } else { usize_ty };
                Operand::Value(self.push_inst(Instruction::SizeOf { ty: target_ty_id }, res_ty))
            }
            Expr::Alignof { target_type } => {
                let target_ty_id = self.resolve_ast_type(target_type);
                let usize_ty = self.ctx.types.usize_id();
                let res_ty = if ty_id != luna_semantic::SemanticTypeId(0) { ty_id } else { usize_ty };
                Operand::Value(self.push_inst(Instruction::AlignOf { ty: target_ty_id }, res_ty))
            }
            Expr::Comptime { .. } => {
                if let Some(ct_val) = self.ctx.comptime_values.get(expr_id).cloned() {
                    self.materialize_comptime_value(&ct_val, ty_id)
                } else {
                    Operand::Number("0".to_string())
                }
            }
            Expr::Cast { expr, target_type } => {
                let val_op = self.generate_expr(expr);
                
                let mut target_ty_id = if ty_id != luna_semantic::SemanticTypeId(0) {
                    ty_id
                } else {
                    self.resolve_ast_type(target_type)
                };
                if target_ty_id == luna_semantic::SemanticTypeId(0) {
                    target_ty_id = self.resolve_ast_type(target_type);
                }
                
                let val = self.push_inst(Instruction::Cast {
                    value: val_op,
                    target_ty: target_ty_id
                }, target_ty_id);
                Operand::Value(val)
            }
            _ => Operand::Number("0".to_string())
        }
    }

    pub fn materialize_comptime_value(&mut self, val: &luna_semantic::ComptimeValue, ty_id: luna_semantic::SemanticTypeId) -> Operand {
        match val {
            luna_semantic::ComptimeValue::Unit => Operand::Number("0".to_string()),
            luna_semantic::ComptimeValue::Bool(b) => Operand::Boolean(*b),
            luna_semantic::ComptimeValue::Int { val, .. } => Operand::Number(val.to_string()),
            luna_semantic::ComptimeValue::Float { val, .. } => Operand::Number(val.to_string()),
            luna_semantic::ComptimeValue::Char(c) => Operand::Number((*c as u32).to_string()),
            luna_semantic::ComptimeValue::Str(s) => Operand::StringRef(format!("\"{}\"", s)),
            luna_semantic::ComptimeValue::Tuple(elements) => {
                let alloca_val = self.push_inst(Instruction::Alloca, ty_id);
                for (i, elem) in elements.iter().enumerate() {
                    let elem_ty = match self.ctx.types.get(ty_id) {
                        luna_semantic::SemanticType::Tuple(tys) => tys.get(i).copied().unwrap_or(luna_semantic::SemanticTypeId(0)),
                        _ => luna_semantic::SemanticTypeId(0),
                    };
                    let elem_op = self.materialize_comptime_value(elem, elem_ty);
                    let ptr = self.push_inst(Instruction::FieldPtr {
                        base: Operand::Value(alloca_val),
                        field_idx: i as u32,
                    }, elem_ty);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(ptr),
                        value: elem_op,
                    }, elem_ty);
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(alloca_val),
                }, ty_id);
                Operand::Value(load_val)
            }
            luna_semantic::ComptimeValue::Array { elements, elem_ty } => {
                let alloca_val = self.push_inst(Instruction::Alloca, ty_id);
                let e_ty = elem_ty.unwrap_or(luna_semantic::SemanticTypeId(0));
                for (i, elem) in elements.iter().enumerate() {
                    let elem_op = self.materialize_comptime_value(elem, e_ty);
                    let ptr = self.push_inst(Instruction::FieldPtr {
                        base: Operand::Value(alloca_val),
                        field_idx: i as u32,
                    }, e_ty);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(ptr),
                        value: elem_op,
                    }, e_ty);
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(alloca_val),
                }, ty_id);
                Operand::Value(load_val)
            }
            luna_semantic::ComptimeValue::Struct { fields, .. } => {
                let alloca_val = self.push_inst(Instruction::Alloca, ty_id);
                for (i, (_, f_val)) in fields.iter().enumerate() {
                    let f_ty = luna_semantic::SemanticTypeId(0);
                    let elem_op = self.materialize_comptime_value(f_val, f_ty);
                    let ptr = self.push_inst(Instruction::FieldPtr {
                        base: Operand::Value(alloca_val),
                        field_idx: i as u32,
                    }, f_ty);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(ptr),
                        value: elem_op,
                    }, f_ty);
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(alloca_val),
                }, ty_id);
                Operand::Value(load_val)
            }
            luna_semantic::ComptimeValue::Enum { variant_index, payload, .. } => {
                let mut payload_ops = Vec::new();
                for p in payload {
                    payload_ops.push(self.materialize_comptime_value(p, luna_semantic::SemanticTypeId(0)));
                }
                let var_val = self.push_inst(Instruction::Variant {
                    enum_ty: ty_id,
                    variant_idx: *variant_index,
                    args: payload_ops,
                }, ty_id);
                Operand::Value(var_val)
            }
            _ => Operand::Number("0".to_string()),
        }
    }
    
    // --- Helpers ---
    
    fn new_label(&mut self, prefix: &str) -> LabelId {
        let id = self.next_label_id;
        self.next_label_id += 1;
        LabelId {
            name: format!("{}{}", prefix, id),
        }
    }
    
    fn start_block(&mut self, label: LabelId) {
        if let Some(mut block) = self.current_block.take() {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator::Br { target: label.clone() });
            }
            self.current_function.as_mut().unwrap().blocks.push(block);
        }
        self.current_block = Some(BasicBlock {
            label,
            insts: Vec::new(),
            terminator: None,
        });
    }
    
    fn extract_expr_span(&self, expr_id: &luna_ast::ExprId) -> Option<luna_common::Span> {
        match &self.arena.exprs[expr_id.0 as usize] {
            luna_ast::Expr::Literal(tok, _) => Some(tok.span.clone()),
            luna_ast::Expr::Identifier { segments, .. } => segments.last().copied(),
            luna_ast::Expr::MethodCall { method_name, .. } => Some(method_name.clone()),
            luna_ast::Expr::Member { member, .. } => Some(member.clone()),
            luna_ast::Expr::Match { match_span, .. } => Some(match_span.clone()),
            luna_ast::Expr::Try { try_span, .. } => Some(try_span.clone()),
            luna_ast::Expr::StructInit { path, .. } => path.last().copied(),
            luna_ast::Expr::Binary { left, .. } => self.extract_expr_span(left),
            luna_ast::Expr::Unary { operand, .. } => self.extract_expr_span(operand),
            luna_ast::Expr::Assign { lvalue, .. } => self.extract_expr_span(lvalue),
            luna_ast::Expr::Call { callee, .. } => self.extract_expr_span(callee),
            luna_ast::Expr::Index { base, .. } => self.extract_expr_span(base),
            luna_ast::Expr::TupleIndex { object, .. } => self.extract_expr_span(object),
            luna_ast::Expr::Cast { expr, .. } => self.extract_expr_span(expr),
            _ => None,
        }
    }

    fn push_inst_span(&mut self, inst: Instruction, ty: luna_semantic::SemanticTypeId, span: Option<luna_common::Span>) -> ValueId {
        let func = self.current_function.as_mut().expect("Must be in a function");
        let val_id = ValueId(func.values.len() as u32);
        let origin = match &inst {
            Instruction::Alloca => ValueOrigin::Local,
            _ => ValueOrigin::Temporary,
        };
        func.values.push(ValueData { inst, ty, span, origin });
        
        if let Some(block) = &mut self.current_block {
            block.insts.push(val_id);
        }
        val_id
    }

    fn push_inst(&mut self, inst: Instruction, ty: luna_semantic::SemanticTypeId) -> ValueId {
        let span = self.current_span.clone();
        self.push_inst_span(inst, ty, span)
    }

    
    fn terminate_block(&mut self, term: Terminator) {
        if let Some(mut block) = self.current_block.take() {
            if block.terminator.is_none() {
                block.terminator = Some(term);
            }
            self.current_function.as_mut().unwrap().blocks.push(block);
        }
    }
    
    fn generate_pat_match(&mut self, pat: &luna_ast::PatId, subject: &Operand) -> Operand {
        use luna_ast::Pattern;
        let pattern = &self.arena.pats[pat.0 as usize];
        match pattern {
            Pattern::Wildcard | Pattern::Identifier { .. } => {
                Operand::Boolean(true)
            }
            Pattern::Enum { path, fields } => {
                let mut variant_idx = 0;
                if let Some(name_span) = path.last() {
                    let name_str = self.get_span_text(*name_span);
                    for symbol in &self.ctx.symbol_table.symbols {
                        if let luna_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                            if symbol.name.ends_with(name_str) {
                                variant_idx = idx;
                                break;
                            }
                        }
                    }
                }
                
                let tag_val = self.push_inst(Instruction::Tag {
                    value: subject.clone(),
                }, self.ctx.types.bool_id());
                
                let expected_tag = Operand::Number(variant_idx.to_string());
                
                let eq_val = self.push_inst(Instruction::Eq {
                    left: Operand::Value(tag_val),
                    right: expected_tag,
                }, self.ctx.types.bool_id());
                
                let mut current_res = Operand::Value(eq_val);
                
                for (field_idx, field_pat) in fields.iter().enumerate() {
                    let extracted = self.push_inst(Instruction::Extract {
                        value: subject.clone(),
                        variant_idx,
                        field_idx: field_idx as u32,
                    }, self.ctx.types.bool_id());
                    
                    let field_match = self.generate_pat_match(field_pat, &Operand::Value(extracted));
                    let and_val = self.push_inst(Instruction::BitAnd {
                        left: current_res.clone(),
                        right: field_match,
                    }, luna_semantic::SemanticTypeId(0));
                    current_res = Operand::Value(and_val);
                }
                
                current_res
            }
            Pattern::Literal(token) => {
                let text = self.get_span_text(token.span).to_string();
                let expected = match token.kind {
                    luna_lexer::TokenKind::KwTrue => Operand::Boolean(true),
                    luna_lexer::TokenKind::KwFalse => Operand::Boolean(false),
                    luna_lexer::TokenKind::StringLiteral => Operand::StringRef(text),
                    luna_lexer::TokenKind::CharLiteral => Operand::Char(text),
                    _ => Operand::Number(text),
                };
                
                let eq_val = self.push_inst(Instruction::Eq {
                    left: subject.clone(),
                    right: expected,
                }, self.ctx.types.bool_id());
                
                Operand::Value(eq_val)
            }
            Pattern::Tuple { elements, .. } => {
                let mut current_res = Operand::Boolean(true);
                for (i, elem) in elements.iter().enumerate() {
                    let extracted = self.push_inst(Instruction::Extract {
                        value: subject.clone(),
                        variant_idx: 0,
                        field_idx: i as u32,
                    }, luna_semantic::SemanticTypeId(0));
                    
                    let field_match = self.generate_pat_match(elem, &Operand::Value(extracted));
                    let and_val = self.push_inst(Instruction::BitAnd {
                        left: current_res.clone(),
                        right: field_match,
                    }, luna_semantic::SemanticTypeId(0));
                    current_res = Operand::Value(and_val);
                }
                current_res
            }
            Pattern::Struct { fields, .. } => {
                let mut current_res = Operand::Boolean(true);
                if let Some(&pat_ty_id) = self.ctx.tables.pat_types.get(pat) {
                    let sem_ty = self.ctx.types.get(pat_ty_id);
                    if let luna_semantic::SemanticType::Struct(sym_id, _, field_tys) = sem_ty {
                        let struct_sym = self.ctx.symbol_table.get_symbol(*sym_id);
                        if let Some(decl_id) = struct_sym.decl_id {
                            if let luna_ast::Decl::Struct { fields: decl_fields, .. } = &self.arena.decls[decl_id.0 as usize] {
                                for struct_field in fields {
                                    let field_name = self.get_span_text(struct_field.name);
                                    let mut found_idx = None;
                                    for (i, f) in decl_fields.iter().enumerate() {
                                        let f_name = self.get_span_text(f.name);
                                        if f_name == field_name {
                                            found_idx = Some(i);
                                            break;
                                        }
                                    }
                                    if let Some(field_idx) = found_idx {
                                        if let Some(ref field_pat) = struct_field.pattern {
                                            let extracted = self.push_inst(Instruction::Extract {
                                                value: subject.clone(),
                                                variant_idx: 0,
                                                field_idx: field_idx as u32,
                                            }, field_tys[field_idx]);
                                            
                                            let field_match = self.generate_pat_match(field_pat, &Operand::Value(extracted));
                                            let and_val = self.push_inst(Instruction::BitAnd {
                                                left: current_res.clone(),
                                                right: field_match,
                                            }, luna_semantic::SemanticTypeId(0));
                                            current_res = Operand::Value(and_val);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                current_res
            }
            _ => Operand::Boolean(false),
        }
    }
    
    fn bind_pat_vars(&mut self, pat: &luna_ast::PatId, subject: &Operand) {
        use luna_ast::Pattern;
        let pattern = &self.arena.pats[pat.0 as usize];
        match pattern {
            Pattern::Identifier { .. } => {
                if let Some(sym_id) = self.ctx.tables.pat_symbols.get(pat).copied() {
                    let symbol = self.ctx.symbol_table.get_symbol(sym_id);
                    if let luna_semantic::SymbolKind::EnumVariant(_) = symbol.kind {
                        // Enum variant matches do not bind a local variable.
                    } else if symbol.name == "_" {
                        // Wildcard identifier "_" does not bind a local variable.
                    } else if let Operand::Value(val_id) = subject {
                        let ty_id = self.get_symbol_type(&sym_id).unwrap_or(luna_semantic::SemanticTypeId(0));
                        let alloca_val = self.push_inst(Instruction::Alloca, ty_id);
                        self.locals.insert(sym_id, alloca_val);
                        if let Some(scope) = self.lexical_scopes.last_mut() {
                            scope.push(sym_id);
                        }
                        
                        self.push_inst(Instruction::Store {
                            ptr: Operand::Value(alloca_val),
                            value: Operand::Value(*val_id),
                        }, ty_id);
                    }
                }
            }
            Pattern::Enum { fields, path } => {
                let mut variant_idx = 0;
                if let Some(name_span) = path.last() {
                    let name_str = self.get_span_text(*name_span);
                    for symbol in &self.ctx.symbol_table.symbols {
                        if let luna_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                            if symbol.name.ends_with(name_str) {
                                variant_idx = idx;
                                break;
                            }
                        }
                    }
                }
                
                for (field_idx, field) in fields.iter().enumerate() {
                    let extracted = self.push_inst(Instruction::Extract {
                        value: subject.clone(),
                        variant_idx,
                        field_idx: field_idx as u32,
                    }, luna_semantic::SemanticTypeId(0));
                    
                    self.bind_pat_vars(field, &Operand::Value(extracted));
                }
            }
            Pattern::Tuple { elements, .. } => {
                for (i, elem) in elements.iter().enumerate() {
                    let extracted = self.push_inst(Instruction::Extract {
                        value: subject.clone(),
                        variant_idx: 0,
                        field_idx: i as u32,
                    }, luna_semantic::SemanticTypeId(0));
                    
                    self.bind_pat_vars(elem, &Operand::Value(extracted));
                }
            }
            Pattern::Struct { fields, .. } => {
                if let Some(&pat_ty_id) = self.ctx.tables.pat_types.get(pat) {
                    let sem_ty = self.ctx.types.get(pat_ty_id);
                    if let luna_semantic::SemanticType::Struct(sym_id, _, field_tys) = sem_ty {
                        let struct_sym = self.ctx.symbol_table.get_symbol(*sym_id);
                        if let Some(decl_id) = struct_sym.decl_id {
                            if let luna_ast::Decl::Struct { fields: decl_fields, .. } = &self.arena.decls[decl_id.0 as usize] {
                                for struct_field in fields {
                                    let field_name = self.get_span_text(struct_field.name);
                                    let mut found_idx = None;
                                    for (i, f) in decl_fields.iter().enumerate() {
                                        let f_name = self.get_span_text(f.name);
                                        if f_name == field_name {
                                            found_idx = Some(i);
                                            break;
                                        }
                                    }
                                    if let Some(field_idx) = found_idx {
                                        if let Some(ref field_pat) = struct_field.pattern {
                                            let extracted = self.push_inst(Instruction::Extract {
                                                value: subject.clone(),
                                                variant_idx: 0,
                                                field_idx: field_idx as u32,
                                            }, field_tys[field_idx]);
                                            
                                            self.bind_pat_vars(field_pat, &Operand::Value(extracted));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
