use crate::{SemanticContext, ty::{SemanticTypeId, SemanticType, BuiltinType}};
use mellis_ast::{AstArena, Item, Stmt, Expr, Decl};
use mellis_lexer::{BuiltinKind, TokenKind};
use mellis_common::diagnostic::Diagnostic;

pub struct TypeChecker<'a> {
    ctx: &'a mut SemanticContext,
    arena: &'a AstArena,
    source: &'a str,
    is_unsafe_context: bool,
    loop_depth: u32,
    active_lambdas: Vec<mellis_ast::ExprId>,
    current_self_type: Option<SemanticTypeId>,
    current_async_fn: Option<mellis_ast::DeclId>,
    comptime_engine: Option<&'a dyn crate::ComptimeEngine>,
    current_return_type: Vec<SemanticTypeId>,
    current_scope: crate::ScopeId,
}

impl<'a> TypeChecker<'a> {
    pub fn new(ctx: &'a mut SemanticContext, arena: &'a AstArena, source: &'a str) -> Self {
        Self { ctx, arena, source, is_unsafe_context: false, loop_depth: 0, active_lambdas: Vec::new(), current_self_type: None, current_async_fn: None, comptime_engine: None, current_return_type: Vec::new(), current_scope: crate::ScopeId(0) }
    }

    pub fn new_with_engine(ctx: &'a mut SemanticContext, arena: &'a AstArena, source: &'a str, comptime_engine: &'a dyn crate::ComptimeEngine) -> Self {
        Self { ctx, arena, source, is_unsafe_context: false, loop_depth: 0, active_lambdas: Vec::new(), current_self_type: None, current_async_fn: None, comptime_engine: Some(comptime_engine), current_return_type: Vec::new(), current_scope: crate::ScopeId(0) }
    }

    pub fn eval_comptime_expr(&mut self, expr_id: mellis_ast::ExprId) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> {
        Err(crate::comptime::ComptimeError::UnsupportedOperation("comptime expressions are not yet supported".to_string()))
    }

    pub fn eval_comptime_stmt(&mut self, stmt_id: mellis_ast::StmtId) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> {
        Err(crate::comptime::ComptimeError::UnsupportedOperation("comptime expressions are not yet supported".to_string()))
    }
    
    pub fn unify(&mut self, expected: SemanticTypeId, actual: SemanticTypeId) -> Result<(), String> {
        let expected = self.ctx.types.resolve(expected);
        let actual = self.ctx.types.resolve(actual);
        
        let t1 = self.ctx.types.get(expected).clone();
        let t2 = self.ctx.types.get(actual).clone();
        
        if expected == actual {
            return Ok(());
        }
        
        match (t1, t2) {
            (SemanticType::InferenceVar(var), _) => {
                if self.ctx.types.occurs_check(var, actual) {
                    return Err(format!("cycle detected during type inference"));
                }
                self.ctx.types.inference_bindings.insert(var, actual);
                Ok(())
            }
            (_, SemanticType::InferenceVar(var)) => {
                if self.ctx.types.occurs_check(var, expected) {
                    return Err(format!("cycle detected during type inference"));
                }
                self.ctx.types.inference_bindings.insert(var, expected);
                Ok(())
            }
            (_, SemanticType::Never) => Ok(()),
            (SemanticType::GenericParam(_), _) => {
                // GenericParam expected, concrete actual — just accept it.
                // The generic param will be resolved to the actual type.
                Ok(())
            }
            (_, SemanticType::GenericParam(_)) => {
                Ok(())
            }
            (SemanticType::Pointer(m1, i1), SemanticType::Pointer(m2, i2)) if m1 == m2 => {
                self.unify(i1, i2)
            }
            (SemanticType::Reference(l1, m1, i1), SemanticType::Reference(l2, m2, i2)) if m1 == m2 && l1 == l2 => {
                self.unify(i1, i2)
            }
            (SemanticType::Reference(_, m1, i1), SemanticType::Pointer(m2, i2)) if m1 == m2 => {
                self.unify(i1, i2)
            }
            (SemanticType::Pointer(m1, i1), SemanticType::Reference(_, m2, i2)) if m1 == m2 => {
                self.unify(i1, i2)
            }
            (SemanticType::Tuple(els1), SemanticType::Tuple(els2)) if els1.len() == els2.len() => {
                for (e1, e2) in els1.into_iter().zip(els2.into_iter()) {
                    self.unify(e1, e2)?;
                }
                Ok(())
            }
            (SemanticType::Array(i1, s1), SemanticType::Array(i2, s2)) if s1 == s2 => {
                self.unify(i1, i2)
            }
            (SemanticType::Slice(i1), SemanticType::Slice(i2)) => {
                self.unify(i1, i2)
            }
            (SemanticType::Struct(s1, args1, _), SemanticType::Struct(s2, args2, _)) if s1 == s2 && args1.len() == args2.len() => {
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    self.unify(*a1, *a2)?;
                }
                Ok(())
            }
            (SemanticType::Enum(e1, args1, _), SemanticType::Enum(e2, args2, _)) if e1 == e2 && args1.len() == args2.len() => {
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    self.unify(*a1, *a2)?;
                }
                Ok(())
            }
            (SemanticType::Function { params: p1, return_type: r1 }, SemanticType::Function { params: p2, return_type: r2 }) if p1.len() == p2.len() => {
                for (a1, a2) in p1.into_iter().zip(p2.into_iter()) {
                    self.unify(a1, a2)?;
                }
                self.unify(r1, r2)
            }
            (SemanticType::Closure(_, p1, r1), SemanticType::Closure(_, p2, r2)) if p1.len() == p2.len() => {
                for (a, b) in p1.into_iter().zip(p2.into_iter()) {
                    self.unify(a, b)?;
                }
                self.unify(r1, r2)
            }
            (SemanticType::Future(o1), SemanticType::Future(o2)) => {
                self.unify(o1, o2)
            }
            _ => Err(format!("type mismatch: expected {:?}, got {:?}", self.ctx.types.get(expected), self.ctx.types.get(actual))),
        }
    }

    pub fn is_ffi_safe(&self, ty_id: SemanticTypeId) -> Result<(), String> {
        let ty = self.ctx.types.get(ty_id).clone();
        match ty {
            SemanticType::Primitive(_) | SemanticType::Void | SemanticType::Never => Ok(()),
            SemanticType::Pointer(_, _inner) | SemanticType::Reference(_, _, _inner) => {
                // The C-ABI doesn't strictly require the pointee to be FFI-safe if it's opaque,
                // but generally it's safer to ensure pointees are also FFI-safe. 
                // We'll just allow it for now since C pointers can point to opaque structs.
                Ok(())
            },
            SemanticType::Struct(sym_id, _, field_tys) => {
                if let Some(decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                    if let Decl::Struct { annotations, .. } = &self.arena.decls[decl_id.0 as usize] {
                        let has_repr_c = annotations.iter().any(|a| {
                            let name = &self.source[a.name.start as usize..a.name.end as usize];
                            name == "repr"
                        });
                        if !has_repr_c {
                            return Err(format!("Struct is not marked with @repr(C)"));
                        }
                    }
                }
                for f in field_tys {
                    self.is_ffi_safe(f)?;
                }
                Ok(())
            },
            SemanticType::Function { params, return_type } => {
                for p in params {
                    self.is_ffi_safe(p)?;
                }
                self.is_ffi_safe(return_type)?;
                Ok(())
            },
            _ => Err(format!("Type is not FFI-safe")),
        }
    }

    pub fn typecheck_items(&mut self, items: &[Item]) {
        self.populate_signatures(items);
        self.check_recursive_types(items);
        self.populate_trait_bounds(items);
        for item in items {
            self.typecheck_item(item);
        }
    }

    fn check_recursive_types(&mut self, items: &[Item]) {
        let mut visiting = std::collections::HashSet::new();
        let mut visited = std::collections::HashSet::new();

        for item in items {
            if let Item::Decl(decl_id) = item {
                if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(decl_id) {
                    if let Some(ty_id) = self.ctx.tables.symbol_types.get(&sym_id).copied() {
                        self.check_recursive_type(ty_id, *decl_id, &mut visiting, &mut visited);
                    }
                }
            }
        }
    }

    fn check_recursive_type(
        &mut self,
        ty_id: SemanticTypeId,
        original_decl: mellis_ast::DeclId,
        visiting: &mut std::collections::HashSet<crate::SymbolId>,
        visited: &mut std::collections::HashSet<crate::SymbolId>,
    ) {
        let ty = self.ctx.types.get(ty_id).clone();
        
        let sym_id_opt = match ty {
            SemanticType::Struct(sym_id, _, _) => Some(sym_id),
            SemanticType::Enum(sym_id, _, _) => Some(sym_id),
            _ => None,
        };

        if let Some(sym_id) = sym_id_opt {
            if visited.contains(&sym_id) {
                return;
            }
            if !visiting.insert(sym_id) {
                let span = match &self.arena.decls[original_decl.0 as usize] {
                    mellis_ast::Decl::Struct { name, .. } => *name,
                    mellis_ast::Decl::Enum { name, .. } => *name,
                    mellis_ast::Decl::TypeAlias { name, .. } => *name,
                    _ => mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0),
                };
                self.ctx.diagnostics.push(Diagnostic::error("Recursive type has infinite size".to_string()).with_span(span));
                return;
            }
        }

        match ty {
            SemanticType::Struct(sym_id, _, _) => {
                if let Some(&full_ty_id) = self.ctx.tables.symbol_types.get(&sym_id) {
                    if let SemanticType::Struct(_, _, full_fields) = self.ctx.types.get(full_ty_id).clone() {
                        for f in full_fields {
                            self.check_recursive_type(f, original_decl, visiting, visited);
                        }
                    }
                }
            }
            SemanticType::Enum(sym_id, _, _) => {
                if let Some(&full_ty_id) = self.ctx.tables.symbol_types.get(&sym_id) {
                    if let SemanticType::Enum(_, _, full_variants) = self.ctx.types.get(full_ty_id).clone() {
                        for v in full_variants {
                            self.check_recursive_type(v, original_decl, visiting, visited);
                        }
                    }
                }
            }
            SemanticType::Tuple(fields) => {
                for f in fields {
                    self.check_recursive_type(f, original_decl, visiting, visited);
                }
            }
            SemanticType::Array(inner, _) => {
                self.check_recursive_type(inner, original_decl, visiting, visited);
            }
            // Indirections do not contribute to infinite size
            SemanticType::Pointer(..) | SemanticType::Reference(..) | SemanticType::Box(..) | SemanticType::Function { .. } | SemanticType::Closure(..) => {}
            _ => {}
        }

        if let Some(sym_id) = sym_id_opt {
            visiting.remove(&sym_id);
            visited.insert(sym_id);
        }
    }

    fn populate_trait_bounds(&mut self, items: &[Item]) {
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                let scope_id = *self.ctx.tables.decl_scopes.get(decl_id).unwrap_or(&crate::ScopeId(0));
                match decl {
                    Decl::Function { generic_params, .. } => {
                        for (gp_idx, gp) in generic_params.iter().enumerate() {
                            let gp_sym_found = self.ctx.tables.generic_param_symbols.get(&(*decl_id, gp_idx));
                            if let Some(gp_sym) = gp_sym_found.copied() {
                                // Register GenericParam type for this symbol
                                let gp_ty = self.ctx.types.intern(SemanticType::GenericParam(gp_sym));
                                self.ctx.tables.symbol_types.insert(gp_sym, gp_ty);
                                for &bound_type_id in &gp.bounds {
                                    if let mellis_ast::Type::Named { segments, .. } = &self.arena.types[bound_type_id.0 as usize] {
                                        if let Some(last_seg) = segments.last() {
                                            let trait_name = &self.source[last_seg.start as usize..last_seg.end as usize];
                                            if let Some(trait_sym) = self.ctx.symbol_table.lookup(trait_name, scope_id)
                                                .or_else(|| self.ctx.symbol_table.lookup(trait_name, crate::ScopeId(0))) {
                                                self.ctx.tables.trait_bounds
                                                    .entry(gp_sym)
                                                    .or_insert_with(Vec::new)
                                                    .push(crate::semantic_tables::TraitBound { param: gp_sym, trait_id: trait_sym });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Decl::Impl { generic_params, methods, .. } => {
                        // Impl-level generic params
                        for (gp_idx, gp) in generic_params.iter().enumerate() {
                            if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(*decl_id, gp_idx)).copied() {
                                let gp_ty = self.ctx.types.intern(SemanticType::GenericParam(gp_sym));
                                self.ctx.tables.symbol_types.insert(gp_sym, gp_ty);
                                for &bound_type_id in &gp.bounds {
                                    if let mellis_ast::Type::Named { segments, .. } = &self.arena.types[bound_type_id.0 as usize] {
                                        if let Some(last_seg) = segments.last() {
                                            let trait_name = &self.source[last_seg.start as usize..last_seg.end as usize];
                                            if let Some(trait_sym) = self.ctx.symbol_table.lookup(trait_name, scope_id)
                                                .or_else(|| self.ctx.symbol_table.lookup(trait_name, crate::ScopeId(0))) {
                                                self.ctx.tables.trait_bounds
                                                    .entry(gp_sym)
                                                    .or_insert_with(Vec::new)
                                                    .push(crate::semantic_tables::TraitBound { param: gp_sym, trait_id: trait_sym });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Method-level generic params
                        for method_decl_id in methods {
                            let m_decl = &self.arena.decls[method_decl_id.0 as usize];
                            let m_scope = *self.ctx.tables.decl_scopes.get(method_decl_id).unwrap_or(&scope_id);
                            if let Decl::Function { generic_params: m_gp, .. } = m_decl {
                                for (gp_idx, gp) in m_gp.iter().enumerate() {
                                    if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(*method_decl_id, gp_idx)).copied() {
                                        let gp_ty = self.ctx.types.intern(SemanticType::GenericParam(gp_sym));
                                        self.ctx.tables.symbol_types.insert(gp_sym, gp_ty);
                                        for &bound_type_id in &gp.bounds {
                                            if let mellis_ast::Type::Named { segments, .. } = &self.arena.types[bound_type_id.0 as usize] {
                                                if let Some(last_seg) = segments.last() {
                                                    let trait_name = &self.source[last_seg.start as usize..last_seg.end as usize];
                                                    let resolved_trait = self.ctx.symbol_table.lookup(trait_name, m_scope)
                                                        .or_else(|| self.ctx.symbol_table.lookup(trait_name, scope_id))
                                                        .or_else(|| self.ctx.symbol_table.lookup(trait_name, crate::ScopeId(0)));
                                                    if let Some(trait_sym) = resolved_trait {
                                                        self.ctx.tables.trait_bounds
                                                            .entry(gp_sym)
                                                            .or_insert_with(Vec::new)
                                                            .push(crate::semantic_tables::TraitBound { param: gp_sym, trait_id: trait_sym });
                                                    }
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
    }

    fn populate_signatures(&mut self, items: &[Item]) {
        // Pass 1: Declare all struct/enum/type types
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Struct { fields, .. } => {
                        let prev_scope = self.current_scope;
                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();
                        if let Some(sym_id) = sym_id_opt {
                            if let Some(inner) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                                self.current_scope = inner;
                            }
                            let dummy_ty = self.ctx.types.intern(SemanticType::Struct(sym_id, vec![], vec![]));
                            self.ctx.tables.symbol_types.insert(sym_id, dummy_ty);
                        }
                        let mut field_tys = Vec::new();
                        let mut seen_fields = std::collections::HashSet::new();
                        for field in fields {
                            let field_name = &self.source[field.name.start as usize..field.name.end as usize];
                            if !seen_fields.insert(field_name) {
                                self.ctx.diagnostics.push(Diagnostic::error(format!("Duplicate field `{}` in struct", field_name)).with_span(field.name));
                            }
                            field_tys.push(self.lower_type(field.ty));
                        }
                        if let Some(sym_id) = sym_id_opt {
                            let struct_ty = self.ctx.types.intern(SemanticType::Struct(sym_id, Vec::new(), field_tys));
                            self.ctx.tables.symbol_types.insert(sym_id, struct_ty);
                        }
                        self.current_scope = prev_scope;
                    }
                    Decl::Enum { generic_params, variants, .. } => {
                        let prev_scope = self.current_scope;
                        let mut enum_args = Vec::new();
                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();
                        if let Some(sym_id) = sym_id_opt {
                            if let Some(inner) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                                self.current_scope = inner;
                            }
                            for (gp_idx, _) in generic_params.iter().enumerate() {
                                if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(*decl_id, gp_idx)) {
                                    enum_args.push(self.ctx.types.intern(SemanticType::GenericParam(*gp_sym)));
                                }
                            }

                            let dummy_ty = self.ctx.types.intern(SemanticType::Enum(sym_id, enum_args.clone(), vec![]));
                            self.ctx.tables.symbol_types.insert(sym_id, dummy_ty);
                        }
                        let mut variant_tys = Vec::new();
                        let mut seen_variants = std::collections::HashSet::new();
                        for variant in variants {
                            let variant_name = &self.source[variant.name.start as usize..variant.name.end as usize];
                            if !seen_variants.insert(variant_name) {
                                self.ctx.diagnostics.push(Diagnostic::error(format!("Duplicate variant `{}` in enum", variant_name)).with_span(variant.name));
                            }
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
                            let enum_ty = self.ctx.types.intern(SemanticType::Enum(sym_id, enum_args, variant_tys));
                            self.ctx.tables.symbol_types.insert(sym_id, enum_ty);
                        }
                        self.current_scope = prev_scope;
                    }
                    _ => {}
                }
            }
        }

        // Pass 1.5: Early evaluation of top-level constants
        let mut const_inits = Vec::new();
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                if let Decl::Var { is_const: true, initializer: Some(init), .. } = decl {
                    const_inits.push((*decl_id, *init));
                }
            }
        }

        for (decl_id, init) in const_inits {
            self.typecheck_expr(&init);
            if let Ok(val) = self.eval_comptime_expr(init) {
                self.ctx.comptime_values.insert(init, val.clone());
                if let Some(sym_id) = self.ctx.tables.decl_symbols.get(&decl_id).copied() {
                    self.ctx.const_values.insert(sym_id, val);
                }
            }
        }

        // Pass 2: Functions and global variables
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { params, return_type, is_async, .. } => {
                        let prev_scope = self.current_scope;
                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();
                        if let Some(sym_id) = sym_id_opt {
                            if let Some(inner) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                                self.current_scope = inner;
                            }
                        }
                        let mut param_tys = Vec::new();
                        for param_id in params {
                            if let Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {
                                let param_ty = if let Some(t) = ty { self.lower_type(*t) } else { self.ctx.types.new_inference_var() };
                                param_tys.push(param_ty);
                                if let Some(param_sym) = self.ctx.tables.decl_symbols.get(param_id) {
                                    self.ctx.tables.symbol_types.insert(*param_sym, param_ty);
                                }
                            }
                        }
                        let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };
                        let actual_ret_ty = if *is_async {
                            self.ctx.types.intern(SemanticType::Future(ret_ty))
                        } else {
                            ret_ty
                        };
                        let func_ty = self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: actual_ret_ty });
                        if let Some(sym_id) = sym_id_opt {
                            self.ctx.tables.symbol_types.insert(sym_id, func_ty);
                            self.ctx.tables.function_effects.entry(sym_id).or_insert_with(crate::effect::EffectSet::pure);
                        }
                        self.current_scope = prev_scope;
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
                    Decl::Trait { methods, .. } => {
                        let prev_scope = self.current_scope;
                        if let Some(&scope) = self.ctx.tables.decl_scopes.get(decl_id) {
                            self.current_scope = scope;
                        }
                        for method_id in methods {
                            let method_decl = &self.arena.decls[method_id.0 as usize];
                            if let Decl::Function { params, return_type, .. } = method_decl {
                                let sym_id_opt = self.ctx.tables.decl_symbols.get(method_id).copied();
                                let mut param_tys = Vec::new();
                                for param_id in params {
                                    if let Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {
                                        let param_ty = if let Some(t) = ty { self.lower_type(*t) } else { self.ctx.types.new_inference_var() };
                                        param_tys.push(param_ty);
                                        if let Some(param_sym) = self.ctx.tables.decl_symbols.get(param_id) {
                                            self.ctx.tables.symbol_types.insert(*param_sym, param_ty);
                                        }
                                    }
                                }
                                let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };
                                let func_ty = self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: ret_ty });
                                if let Some(sym_id) = sym_id_opt {
                                    self.ctx.tables.symbol_types.insert(sym_id, func_ty);
                                }
                            }
                        }
                        self.current_scope = prev_scope;
                    }
                    Decl::Impl { generic_params, self_type, methods, .. } => {
                        if true {
                            let prev_scope = self.current_scope;
                            let impl_scope = *self.ctx.tables.decl_scopes.get(decl_id).unwrap_or(&crate::ScopeId(0));
                            self.current_scope = impl_scope;
                            let self_sem_ty = self.lower_type(*self_type);
                            let prev_self = self.current_self_type;
                            self.current_self_type = Some(self_sem_ty);
                            for method_id in methods {
                                let method_decl = &self.arena.decls[method_id.0 as usize];
                                if let Decl::Function { params, return_type, is_async, .. } = method_decl {
                                    let sym_id_opt = self.ctx.tables.decl_symbols.get(method_id).copied();
                                    // Enter method scope to see method-level generic params
                                    if let Some(sym_id) = sym_id_opt {
                                        if let Some(inner) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                                            self.current_scope = inner;
                                        }
                                    }
                                    let mut param_tys = Vec::new();
                                    for param_id in params {
                                        if let Decl::Param { ty, is_self, .. } = &self.arena.decls[param_id.0 as usize] {
                                            let param_ty = if let Some(t) = ty {
                                                self.lower_type(*t)
                                            } else if *is_self {
                                                self_sem_ty
                                            } else {
                                                self.ctx.types.new_inference_var()
                                            };
                                            param_tys.push(param_ty);
                                            if let Some(param_sym) = self.ctx.tables.decl_symbols.get(param_id) {
                                                self.ctx.tables.symbol_types.insert(*param_sym, param_ty);
                                            }
                                        }
                                    }
                                    let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };
                                    let actual_ret_ty = if *is_async {
                                        self.ctx.types.intern(SemanticType::Future(ret_ty))
                                    } else {
                                        ret_ty
                                    };
                                    let func_ty = self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: actual_ret_ty });
                                    if let Some(sym_id) = sym_id_opt {
                                        self.ctx.tables.symbol_types.insert(sym_id, func_ty);
                                    }
                                }
                            }
                            self.current_self_type = prev_self;
                            self.current_scope = prev_scope;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Pass 3: Check Drop Impls & Trait Impls
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                if let Decl::Impl { trait_type, self_type, methods, .. } = decl {
                    let self_sym_opt = self.ctx.tables.type_symbols.get(self_type).copied().or_else(|| {
                        let self_ast_ty = &self.arena.types[self_type.0 as usize];
                        if let mellis_ast::Type::Named { segments, .. } = self_ast_ty {
                            if segments.len() == 1 {
                                let s = &segments[0];
                                let name = &self.source[s.start as usize..s.end as usize];
                                self.ctx.symbol_table.lookup_with_ctxt(name, s.ctxt, crate::ScopeId(0))
                            } else {
                                let mut scope = crate::ScopeId(0);
                                let mut res = None;
                                for (i, seg) in segments.iter().enumerate() {
                                    let seg_name = &self.source[seg.start as usize..seg.end as usize];
                                    let sym_id = if i == 0 {
                                        self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, scope)
                                    } else {
                                        self.ctx.symbol_table.lookup_exact_with_ctxt(seg_name, seg.ctxt, scope)
                                    };
                                    if let Some(id) = sym_id {
                                        res = Some(id);
                                        if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                            scope = inner;
                                        } else if i < segments.len() - 1 {
                                            res = None;
                                            break;
                                        }
                                    } else {
                                        res = None;
                                        break;
                                    }
                                }
                                res
                            }
                        } else {
                            None
                        }
                    });

                    if let Some(trait_ty_id) = trait_type {
                        let trait_sym_opt = self.ctx.tables.type_symbols.get(trait_ty_id).copied().or_else(|| {
                            let trait_ast_ty = &self.arena.types[trait_ty_id.0 as usize];
                            if let mellis_ast::Type::Named { segments, .. } = trait_ast_ty {
                                if segments.len() == 1 {
                                    let s = &segments[0];
                                    let name = &self.source[s.start as usize..s.end as usize];
                                    self.ctx.symbol_table.lookup_with_ctxt(name, s.ctxt, crate::ScopeId(0))
                                } else {
                                    let mut scope = crate::ScopeId(0);
                                    let mut res = None;
                                    for (i, seg) in segments.iter().enumerate() {
                                        let seg_name = &self.source[seg.start as usize..seg.end as usize];
                                        let sym_id = if i == 0 {
                                            self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, scope)
                                        } else {
                                            self.ctx.symbol_table.lookup_exact_with_ctxt(seg_name, seg.ctxt, scope)
                                        };
                                        if let Some(id) = sym_id {
                                            res = Some(id);
                                            if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                                scope = inner;
                                            } else if i < segments.len() - 1 {
                                                res = None;
                                                break;
                                            }
                                        } else {
                                            res = None;
                                            break;
                                        }
                                    }
                                    res
                                }
                            } else {
                                None
                            }
                        });

                        if let Some(trait_sym) = trait_sym_opt {
                            if Some(trait_sym) == self.ctx.lang_items.get(crate::lang_item::LangItem::Drop) {
                                if let Some(sym_id) = self_sym_opt {
                                    if let Some(&drop_meth_decl) = methods.first() {
                                        if let Some(&drop_meth_sym) = self.ctx.tables.decl_symbols.get(&drop_meth_decl) {
                                            self.ctx.tables.drop_impls.insert(sym_id, drop_meth_sym);
                                        }
                                    }
                                }
                            } else {
                                if let Some(self_sym) = self_sym_opt {
                                    self.ctx.tables.trait_impls
                                        .entry(crate::semantic_tables::ImplKey {
                                            trait_id: Some(trait_sym),
                                            self_type_def: self_sym,
                                        })
                                        .or_insert_with(Vec::new)
                                        .push(*decl_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }


    fn check_bounds_for_call(&mut self, func_sym: mellis_common::ids::SymbolId, subst: &crate::ty::Substitution, span: mellis_common::Span) {
        if let Some(&decl_id) = self.ctx.tables.symbol_decls.get(&func_sym) {
            let mut i = 0;
            loop {
                let gp_sym = match self.ctx.tables.generic_param_symbols.get(&(decl_id, i)) {
                    Some(&s) => s,
                    None => break,
                };
                // Get the concrete type that was inferred for this generic param
                let inferred_ty_opt = subst.get(gp_sym).copied();
                if let Some(inferred_ty) = inferred_ty_opt {
                    let resolved_ty = self.ctx.types.resolve(inferred_ty);
                    let resolved_sem = self.ctx.types.get(resolved_ty).clone();
                    // Skip if still an inference var or generic param
                    if matches!(resolved_sem, SemanticType::InferenceVar(_) | SemanticType::GenericParam(_)) {
                        i += 1;
                        continue;
                    }
                    // Find the struct symbol for the resolved type (peel through references/pointers)
                    let concrete_struct_sym = match &resolved_sem {
                        SemanticType::Struct(s, _, _) | SemanticType::Enum(s, _, _) => Some(*s),
                        SemanticType::Reference(_, _, inner) | SemanticType::Pointer(_, inner) => {
                            let inner_ty = self.ctx.types.get(*inner);
                            if let SemanticType::Struct(s, _, _) | SemanticType::Enum(s, _, _) = inner_ty {
                                Some(*s)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let (Some(concrete_sym), Some(bounds)) = (concrete_struct_sym, self.ctx.tables.trait_bounds.get(&gp_sym).cloned()) {
                        for bound in &bounds {
                            let impl_key = crate::semantic_tables::ImplKey {
                                trait_id: Some(bound.trait_id),
                                self_type_def: concrete_sym,
                            };
                            if !self.ctx.tables.trait_impls.contains_key(&impl_key) {
                                let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();
                                let type_name = self.ctx.symbol_table.get_symbol(concrete_sym).name.clone();
                                let gp_name = self.ctx.symbol_table.get_symbol(gp_sym).name.clone();
                                self.ctx.diagnostics.push(
                                    Diagnostic::error(format!(
                                        "The type `{}` does not implement trait `{}` (required by inferred generic parameter `{}`)",
                                        type_name, trait_name, gp_name
                                    )).with_span(span)
                                );
                            }
                        }
                    }
                }
                i += 1;
            }
        }
    }

    /// Walk an AST type to find GenericParam references and map them to concrete types from arguments.

    pub fn check_object_safety(&mut self, trait_sym: mellis_common::ids::SymbolId, span: Option<mellis_common::Span>) -> bool {
        let Some(&decl_id) = self.ctx.tables.symbol_decls.get(&trait_sym) else {
            return true;
        };
        let Decl::Trait { methods, .. } = &self.arena.decls[decl_id.0 as usize] else {
            return true;
        };
        for &method_decl_id in methods {
            let method_decl = &self.arena.decls[method_decl_id.0 as usize];
            if let Decl::Function { name, generic_params, params, return_type, .. } = method_decl {
                let method_name = &self.source[name.start as usize..name.end as usize];
                
                // Rule 1: No generic parameters
                if !generic_params.is_empty() {
                    let mut diag = Diagnostic::error(format!(
                        "Trait `{}` cannot be made into an object because method `{}` has generic type parameters",
                        self.ctx.symbol_table.get_symbol(trait_sym).name,
                        method_name
                    ));
                    if let Some(sp) = span { diag = diag.with_span(sp); }
                    self.ctx.diagnostics.push(diag);
                    return false;
                }
                
                // Rule 2: Must have receiver (`self`, `&self`, or `&rw self`)
                if params.is_empty() {
                    let mut diag = Diagnostic::error(format!(
                        "Trait `{}` cannot be made into an object because method `{}` has no `self` receiver",
                        self.ctx.symbol_table.get_symbol(trait_sym).name,
                        method_name
                    ));
                    if let Some(sp) = span { diag = diag.with_span(sp); }
                    self.ctx.diagnostics.push(diag);
                    return false;
                }
                let first_param = &self.arena.decls[params[0].0 as usize];
                if let Decl::Param { is_self, .. } = first_param {
                    if !*is_self {
                        let mut diag = Diagnostic::error(format!(
                            "Trait `{}` cannot be made into an object because method `{}` has no `self` receiver",
                            self.ctx.symbol_table.get_symbol(trait_sym).name,
                            method_name
                        ));
                        if let Some(sp) = span { diag = diag.with_span(sp); }
                        self.ctx.diagnostics.push(diag);
                        return false;
                    }
                }
                
                // Rule 3: Return type cannot be unboxed `Self`
                if let Some(ret_ty_id) = return_type {
                    let ret_ast = &self.arena.types[ret_ty_id.0 as usize];
                    if let mellis_ast::Type::Named { segments, .. } = ret_ast {
                        if let Some(last_seg) = segments.last() {
                            let ret_name = &self.source[last_seg.start as usize..last_seg.end as usize];
                            if ret_name == "Self" {
                                let mut diag = Diagnostic::error(format!(
                                    "Trait `{}` cannot be made into an object because method `{}` returns `Self`",
                                    self.ctx.symbol_table.get_symbol(trait_sym).name,
                                    method_name
                                ));
                                if let Some(sp) = span { diag = diag.with_span(sp); }
                                self.ctx.diagnostics.push(diag);
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    pub fn try_coerce_dyn(&mut self, expr_id: mellis_ast::ExprId, from_ty: SemanticTypeId, to_ty: SemanticTypeId) -> bool {
        let from = self.ctx.types.get(from_ty).clone();
        let to = self.ctx.types.get(to_ty).clone();
        
        let (from_inner, to_trait) = match (&from, &to) {
            (SemanticType::Pointer(_, f_in), SemanticType::Pointer(_, t_in))
            | (SemanticType::Reference(_, _, f_in), SemanticType::Reference(_, _, t_in))
            | (SemanticType::Pointer(_, f_in), SemanticType::Reference(_, _, t_in))
            | (SemanticType::Reference(_, _, f_in), SemanticType::Pointer(_, t_in)) => {
                let t_ty = self.ctx.types.get(*t_in).clone();
                if let SemanticType::DynTrait(trait_sym) = t_ty {
                    (*f_in, trait_sym)
                } else {
                    return false;
                }
            }
            (_, SemanticType::DynTrait(trait_sym)) => {
                (from_ty, *trait_sym)
            }
            _ => return false,
        };
        
        let f_ty = self.ctx.types.get(from_inner).clone();
        let concrete_sym = match f_ty {
            SemanticType::Struct(s_sym, _, _) => s_sym,
            SemanticType::Enum(e_sym, _, _) => e_sym,
            _ => return false,
        };
        
        let key = crate::semantic_tables::ImplKey {
            trait_id: Some(to_trait),
            self_type_def: concrete_sym,
        };
        if self.ctx.tables.trait_impls.contains_key(&key) {
            self.ctx.tables.dyn_coercions.insert(expr_id, (to_trait, concrete_sym));
            return true;
        }
        false
    }

    pub fn lower_type(&mut self, ast_ty_id: mellis_ast::TypeId) -> SemanticTypeId {
        use mellis_ast::Type;
        let ast_ty = &self.arena.types[ast_ty_id.0 as usize];
        let sem_ty_id = match ast_ty {
            Type::Builtin(kind) => {
                let builtin = match kind {
                    BuiltinKind::I8 => BuiltinType::I8,
                    BuiltinKind::I16 => BuiltinType::I16,
                    BuiltinKind::I32 => BuiltinType::I32,
                    BuiltinKind::I64 => BuiltinType::I64,
                    BuiltinKind::I128 => BuiltinType::I128,
                    BuiltinKind::Isize => BuiltinType::Isize,
                    BuiltinKind::U8 => BuiltinType::U8,
                    BuiltinKind::U16 => BuiltinType::U16,
                    BuiltinKind::U32 => BuiltinType::U32,
                    BuiltinKind::U64 => BuiltinType::U64,
                    BuiltinKind::U128 => BuiltinType::U128,
                    BuiltinKind::Usize => BuiltinType::Usize,
                    BuiltinKind::F32 => BuiltinType::F32,
                    BuiltinKind::F64 => BuiltinType::F64,
                    BuiltinKind::Bool => BuiltinType::Bool,
                    BuiltinKind::Str => BuiltinType::String,
                    BuiltinKind::Char => BuiltinType::Char,
                    BuiltinKind::Void => return self.ctx.types.intern(SemanticType::Void),
                };
                self.ctx.types.intern(SemanticType::Primitive(builtin))
            }
            Type::Named { segments, generic_args, .. } => {
                let name = segments.last().map(|span| &self.source[span.start as usize..span.end as usize]);
                if let Some("Self") = name {
                    if let Some(self_ty) = self.current_self_type {
                        return self_ty;
                    } else {
                        return self.ctx.types.new_inference_var();
                    }
                }
                if segments.len() >= 2 {
                    let first_seg = &self.source[segments[0].start as usize..segments[0].end as usize];
                    if first_seg == "Self" {
                        // Associated type projection like Self::Output or Self::Residual
                        return self.ctx.types.new_inference_var();
                    }
                }
                if let Some("Future") = name {
                    if let Some(&arg) = generic_args.first() {
                        let out_ty = self.lower_type(arg);
                        return self.ctx.types.intern(SemanticType::Future(out_ty));
                    }
                }
                let last_span = segments.last().copied();
                let ctxt = last_span.map(|s| s.ctxt).unwrap_or(mellis_common::ids::SyntaxContext::ROOT);
                // Check if already resolved by Resolver, or resolve path
                let symbol = self.ctx.tables.type_symbols.get(&ast_ty_id).copied().or_else(|| {
                    if segments.len() == 1 {
                        name.and_then(|n| {
                            self.ctx.symbol_table.lookup_with_ctxt(n, ctxt, self.current_scope)
                                .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(n, ctxt, crate::ScopeId(0)))
                        })
                    } else {
                        let mut scope = self.current_scope;
                        let mut res = None;
                        for (i, seg) in segments.iter().enumerate() {
                            let seg_name = &self.source[seg.start as usize..seg.end as usize];
                            let sym_id = if i == 0 {
                                self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, scope)
                                    .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, crate::ScopeId(0)))
                            } else {
                                self.ctx.symbol_table.lookup_exact_with_ctxt(seg_name, seg.ctxt, scope)
                            };
                            if let Some(id) = sym_id {
                                res = Some(id);
                                if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                    scope = inner;
                                } else if i < segments.len() - 1 {
                                    res = None;
                                    break;
                                }
                            } else {
                                res = None;
                                break;
                            }
                        }
                        res
                    }
                });
                if let Some(sym) = symbol {
                    let sym_kind = self.ctx.symbol_table.get_symbol(sym).kind.clone();
                    if let crate::SymbolKind::TypeParam = sym_kind {
                        // This is a generic type parameter - return GenericParam type
                        self.ctx.types.intern(SemanticType::GenericParam(sym))
                    } else {
                        let base_ty = self.ctx.tables.symbol_types.get(&sym).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());
                        if generic_args.is_empty() {
                            base_ty
                        } else {
                            if let Some(decl_id) = self.ctx.tables.symbol_decls.get(&sym).copied() {
                                let decl = &self.arena.decls[decl_id.0 as usize];
                                let generic_params = match decl {
                                    mellis_ast::Decl::Struct { generic_params, .. } => Some(generic_params),
                                    mellis_ast::Decl::Enum { generic_params, .. } => Some(generic_params),
                                    _ => None,
                                };
                                if let Some(gp_list) = generic_params {
                                    let mut concrete_args = Vec::new();
                                    for arg in generic_args {
                                        concrete_args.push(self.lower_type(*arg));
                                    }
                                    let mut subst = crate::ty::Substitution::new();
                                    for (idx, _gp) in gp_list.iter().enumerate() {
                                        if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(decl_id, idx)) {
                                            if idx < concrete_args.len() {
                                                subst.insert(*gp_sym, concrete_args[idx]);
                                            }
                                        }
                                    }
                                    
                                    let resolved_ty = self.ctx.types.get(base_ty).clone();
                                    match resolved_ty {
                                        SemanticType::Struct(s_sym, _, original_field_tys) => {
                                            let mut field_tys = Vec::new();
                                            for orig_ty in original_field_tys {
                                                field_tys.push(self.ctx.types.subst(orig_ty, &subst));
                                            }
                                            self.ctx.types.intern(SemanticType::Struct(s_sym, concrete_args, field_tys))
                                        }
                                        SemanticType::Enum(e_sym, _, original_var_tys) => {
                                            let mut variant_tys = Vec::new();
                                            for orig_ty in original_var_tys {
                                                variant_tys.push(self.ctx.types.subst(orig_ty, &subst));
                                            }
                                            self.ctx.types.intern(SemanticType::Enum(e_sym, concrete_args, variant_tys))
                                        }
                                        _ => base_ty,
                                    }
                                } else {
                                    base_ty
                                }
                            } else {
                                base_ty
                            }
                        }
                    }
                } else {
                    let name_str = segments.iter().map(|s| &self.source[s.start as usize..s.end as usize]).collect::<Vec<_>>().join("::");
                    let span = mellis_common::Span {
                        file_id: segments[0].file_id,
                        start: segments[0].start,
                        end: segments.last().unwrap().end,
                        ctxt: segments[0].ctxt,
                    };
                    self.ctx.diagnostics.push(mellis_common::Diagnostic::error(format!("cannot find type `{}` in this scope", name_str)).with_span(span));
                    self.ctx.types.intern(SemanticType::Error)
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
                let resolved_size = match self.eval_comptime_expr(*size) {
                    Ok(val) => val.as_usize().unwrap_or(0) as u64,
                    Err(e) => {
                        let span = self.get_expr_span_for_diag(size).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                        self.ctx.diagnostics.push(Diagnostic::error(format!("cannot evaluate array size in comptime: {}", e)).with_span(span));
                        0
                    }
                };
                self.ctx.types.intern(SemanticType::Array(inner_ty, resolved_size))
            }
            Type::Reference { is_mutable, inner, .. } => {
                let inner_ty = self.lower_type(*inner);
                let mutability = if *is_mutable { crate::ty::Mutability::Mutable } else { crate::ty::Mutability::Immutable };
                self.ctx.types.intern(SemanticType::Reference(crate::ty::LifetimeId(0), mutability, inner_ty))
            }
            Type::Pointer { is_mutable, inner, .. } => {
                let inner_ty = self.lower_type(*inner);
                let mutability = if *is_mutable { crate::ty::Mutability::Mutable } else { crate::ty::Mutability::Immutable };
                self.ctx.types.intern(SemanticType::Pointer(mutability, inner_ty))
            }
            Type::TraitObject { trait_type } => {
                let trait_ast = &self.arena.types[trait_type.0 as usize];
                if let Type::Named { segments, .. } = trait_ast {
                    let name = segments.last().map(|span| &self.source[span.start as usize..span.end as usize]);
                    if let Some(trait_sym) = name.and_then(|n| self.ctx.symbol_table.lookup(n, crate::ScopeId(0))) {
                        if !self.check_object_safety(trait_sym, segments.last().copied()) {
                            return self.ctx.types.intern(SemanticType::Error);
                        }
                        self.ctx.types.intern(SemanticType::DynTrait(trait_sym))
                    } else {
                        self.ctx.diagnostics.push(Diagnostic::error("Trait not found for dyn Trait"));
                        self.ctx.types.intern(SemanticType::Error)
                    }
                } else {
                    self.ctx.types.intern(SemanticType::Error)
                }
            }
            Type::MacroCall { span, .. } => {
                self.ctx.diagnostics.push(Diagnostic::error("internal compiler error: unexpanded macro call in type position").with_span(*span));
                self.ctx.types.intern(SemanticType::Error)
            }
            Type::Typeof { expr } => {
                self.typecheck_expr(expr)
            }
            Type::Never => {
                self.ctx.types.intern(SemanticType::Never)
            }
            other => {
                self.ctx.diagnostics.push(Diagnostic::error(format!("Unsupported or unrecognized type construct in semantic phase: {:?}", other)));
                self.ctx.types.intern(SemanticType::Error)
            }
        };
        self.ctx.tables.ast_type_to_semantic.insert(ast_ty_id, sem_ty_id);
        sem_ty_id
    }

    fn typecheck_item(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { body, is_async, return_type, .. } => {
                        let prev_scope = self.current_scope;
                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(decl_id).copied() {
                            if let Some(inner) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                                self.current_scope = inner;
                            }
                        }
                        let prev_async = self.current_async_fn;
                        if *is_async {
                            self.current_async_fn = Some(*decl_id);
                        } else {
                            self.current_async_fn = None;
                        }
                        let ret_ty = if let Some(r) = return_type {
                            self.lower_type(*r)
                        } else {
                            self.ctx.types.intern(SemanticType::Void)
                        };
                        self.current_return_type.push(ret_ty);
                        if let Some(body_stmt) = body {
                            self.typecheck_stmt(body_stmt);
                        }
                        self.current_return_type.pop();
                        self.current_async_fn = prev_async;
                        self.current_scope = prev_scope;
                    }
                    Decl::Var { name, initializer, pattern, type_annot, is_const, .. } => {
                        let mut init_ty = if let Some(init) = initializer {
                            self.typecheck_expr(init)
                        } else {
                            self.ctx.types.new_inference_var()
                        };
                        
                        if let Some(annot) = type_annot {
                            let expected_ty = self.lower_type(*annot);
                            if let Some(init) = initializer {
                                if !self.try_coerce_dyn(*init, init_ty, expected_ty) {
                                    if let Err(e) = self.unify(expected_ty, init_ty) {
                                    let span = self.get_expr_span_for_diag(init).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                    self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                }
                                } else {
                                    init_ty = expected_ty;
                                }
                            }
                        }
                        
                        if *is_const {
                            if let Some(init) = initializer {
                                match self.eval_comptime_expr(*init) {
                                    Ok(val) => {
                                        self.ctx.comptime_values.insert(*init, val.clone());
                                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(decl_id).copied() {
                                            self.ctx.const_values.insert(sym_id, val);
                                        }
                                    }
                                    Err(e) => {
                                        self.ctx.diagnostics.push(Diagnostic::error(format!("cannot evaluate constant in comptime: {}", e)).with_span(*name));
                                    }
                                }
                            }
                        }

                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(decl_id).copied() {
                            let mut needs_update = false;
                            if let Some(current_ty) = self.ctx.tables.symbol_types.get(&sym_id).copied() {
                                let resolved = self.ctx.types.get(current_ty).clone();
                                if matches!(resolved, SemanticType::InferenceVar(_)) {
                                    needs_update = true;
                                }
                            } else {
                                needs_update = true;
                            }
                            if needs_update {
                                self.ctx.tables.symbol_types.insert(sym_id, init_ty);
                            }
                        }
                        
                        if let Some(pat_id) = pattern {
                            self.typecheck_pattern(pat_id, init_ty);
                        }
                    }
                    Decl::Impl { generic_params, self_type, methods, .. } => {
                        if generic_params.is_empty() {
                            let prev_scope = self.current_scope;
                            let impl_scope = *self.ctx.tables.decl_scopes.get(decl_id).unwrap_or(&crate::ScopeId(0));
                            self.current_scope = impl_scope;
                            let self_sem_ty = self.lower_type(*self_type);
                            let prev_self = self.current_self_type;
                            self.current_self_type = Some(self_sem_ty);
                            for method_id in methods {
                                let method_decl = &self.arena.decls[method_id.0 as usize];
                                if let Decl::Function { body: Some(body_stmt), is_async, return_type, .. } = method_decl {
                                    // Enter method scope
                                    if let Some(sym_id) = self.ctx.tables.decl_symbols.get(method_id).copied() {
                                        if let Some(inner) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                                            self.current_scope = inner;
                                        }
                                    }
                                    let prev_async = self.current_async_fn;
                                    if *is_async {
                                        self.current_async_fn = Some(*method_id);
                                    } else {
                                        self.current_async_fn = None;
                                    }
                                    let ret_ty = if let Some(r) = return_type {
                                        self.lower_type(*r)
                                    } else {
                                        self.ctx.types.intern(SemanticType::Void)
                                    };
                                    self.current_return_type.push(ret_ty);
                                    self.typecheck_stmt(body_stmt);
                                    self.current_return_type.pop();
                                    self.current_async_fn = prev_async;
                                }
                            }
                            self.current_self_type = prev_self;
                            self.current_scope = prev_scope;
                        }
                    }
                    Decl::Extern { func, .. } => {
                        // The inner func is registered, we must check its FFI safety
                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(func).copied() {
                            let mut eff = crate::effect::EffectSet::pure();
                            eff.add(crate::effect::Effect::Extern);
                            eff.add(crate::effect::Effect::IO);
                            self.ctx.tables.function_effects.insert(sym_id, eff);

                            if let Some(func_ty) = self.ctx.tables.symbol_types.get(&sym_id).copied() {
                                if let Err(e) = self.is_ffi_safe(func_ty) {
                                    self.ctx.diagnostics.push(Diagnostic::error(format!("Extern function signature is not FFI-safe: {}", e)));
                                }
                            }
                        }
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
                    if elements.len() != elem_tys.len() {
                        self.ctx.diagnostics.push(Diagnostic::error(format!("Tuple pattern has {} elements, but tuple type has {}", elements.len(), elem_tys.len())));
                    }
                    for (i, elem) in elements.iter().enumerate() {
                        if i < elem_tys.len() {
                            self.typecheck_pattern(elem, elem_tys[i]);
                        }
                    }
                }
            }
            mellis_ast::Pattern::Struct { fields, path, has_rest } => {
                let resolved_ty = self.ctx.types.get(ty).clone();
                if let SemanticType::Struct(sym_id, _, _) = resolved_ty {
                    let decl_id_opt = self.ctx.symbol_table.get_symbol(sym_id).decl_id;
                    if let Some(decl_id) = decl_id_opt {
                        let decl = self.arena.decls[decl_id.0 as usize].clone();
                        if let mellis_ast::Decl::Struct { fields: struct_fields, .. } = &decl {
                            let mut pat_span = mellis_common::ids::Span::new(mellis_common::ids::FileId(0), 0, 0);
                            if let Some(first) = path.first() { pat_span = *first; }
                            
                            // Check for missing fields if has_rest is false
                            let mut missing_fields = false;
                            if !*has_rest {
                                for struct_field in struct_fields {
                                    let struct_field_name = &self.source[struct_field.name.start as usize..struct_field.name.end as usize];
                                    let mut provided = false;
                                    for field in fields {
                                        let field_name_str = &self.source[field.name.start as usize..field.name.end as usize];
                                        if field_name_str == struct_field_name { provided = true; break; }
                                    }
                                    if !provided {
                                        self.ctx.diagnostics.push(mellis_common::diagnostic::Diagnostic::error(format!("Pattern requires field `{}` but it was not provided", struct_field_name)).with_span(pat_span));
                                        missing_fields = true;
                                    }
                                }
                            }
                            if missing_fields {
                                return;
                            }

                            for field in fields {
                                if let Some(field_pat) = field.pattern {
                                    let field_name_str = &self.source[field.name.start as usize..field.name.end as usize];
                                    
                                    let mut field_ty = self.ctx.types.new_inference_var();
                                    let mut found = false;
                                    for struct_field in struct_fields {
                                        let struct_field_name = &self.source[struct_field.name.start as usize..struct_field.name.end as usize];
                                        if field_name_str == struct_field_name {
                                            field_ty = self.ctx.tables.ast_type_to_semantic.get(&struct_field.ty).copied().unwrap_or(field_ty);
                                            found = true;
                                            break;
                                        }
                                    }
                                    
                                    if !found {
                                        self.ctx.diagnostics.push(mellis_common::diagnostic::Diagnostic::error(format!("Struct has no field named `{}`", field_name_str)).with_span(field.name));
                                    }
                                    
                                    self.typecheck_pattern(&field_pat, field_ty);
                                }
                            }
                        }
                    }
                }
            }
            mellis_ast::Pattern::Enum { fields, path } => {
                let mut variant_payload_tys = Vec::new();
                if let Some(variant_sym_id) = self.ctx.tables.pat_symbols.get(pat_id).copied() {
                    let variant_sym = self.ctx.symbol_table.get_symbol(variant_sym_id);
                    if let crate::symbol::SymbolKind::EnumVariant(variant_idx) = variant_sym.kind {
                        if let Some(decl_id) = variant_sym.decl_id {
                            if let mellis_ast::Decl::Enum { variants, .. } = &self.arena.decls[decl_id.0 as usize] {
                                if let Some(variant) = variants.get(variant_idx as usize) {
                                    for &param_decl_id in &variant.fields {
                                        if let mellis_ast::Decl::Param { ty: Some(ty_id), .. } = &self.arena.decls[param_decl_id.0 as usize] {
                                            let sem_ty = self.ctx.tables.ast_type_to_semantic.get(ty_id).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());
                                            variant_payload_tys.push(sem_ty);
                                        } else {
                                            variant_payload_tys.push(self.ctx.types.new_inference_var());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                if fields.len() != variant_payload_tys.len() {
                    let mut pat_span = mellis_common::ids::Span::new(mellis_common::ids::FileId(0), 0, 0);
                    if let Some(first) = path.first() { pat_span = *first; }
                    self.ctx.diagnostics.push(mellis_common::diagnostic::Diagnostic::error(format!("Enum variant expects {} fields, but {} were provided", variant_payload_tys.len(), fields.len())).with_span(pat_span));
                    return;
                }

                for (i, field) in fields.iter().enumerate() {
                    let field_ty = variant_payload_tys.get(i).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());
                    self.typecheck_pattern(field, field_ty);
                }
            }
            _ => {}
        }
    }

    fn infer_stmt_value_type(&self, stmt_id: &mellis_ast::StmtId) -> Option<SemanticTypeId> {
        match &self.arena.stmts[stmt_id.0 as usize] {
            Stmt::Block { body, tail_expr } => {
                if let Some(expr) = tail_expr {
                    return self.ctx.tables.expr_types.get(expr).copied();
                }
                for item in body.iter().rev() {
                    if let Item::Stmt(stmt) = item {
                        if let Some(ty) = self.infer_stmt_value_type(stmt) {
                            return Some(ty);
                        }
                    }
                }
                None
            }
            Stmt::Return { value: Some(expr) } => self.ctx.tables.expr_types.get(expr).copied(),
            Stmt::If { then_branch, else_branch: Some(else_branch), .. } => {
                self.infer_stmt_value_type(then_branch).or_else(|| self.infer_stmt_value_type(else_branch))
            }
            _ => None,
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
                let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));
                if self.unify(bool_ty, cond_ty).is_err() {
                    self.ctx.diagnostics.push(Diagnostic::error("if condition must be a boolean"));
                }
                self.typecheck_stmt(then_branch);
                if let Some(else_br) = else_branch {
                    self.typecheck_stmt(else_br);
                }
            }
            Stmt::While { condition, body, .. } => {
                let cond_ty = self.typecheck_expr(condition);
                let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));
                if self.unify(bool_ty, cond_ty).is_err() {
                    self.ctx.diagnostics.push(Diagnostic::error("while condition must be a boolean"));
                }
                self.loop_depth += 1;
                self.typecheck_stmt(body);
                self.loop_depth -= 1;
            }
            Stmt::For { init, cond, step, body, iterable, .. } => {
                if let Some(item) = init { self.typecheck_item(item); }
                if let Some(c) = cond { 
                    let cond_ty = self.typecheck_expr(c); 
                    let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));
                    if self.unify(bool_ty, cond_ty).is_err() {
                        self.ctx.diagnostics.push(Diagnostic::error("for condition must be a boolean"));
                    }
                }
                if let Some(s) = step { self.typecheck_expr(s); }
                if let Some(iter) = iterable { self.typecheck_expr(iter); }
                self.loop_depth += 1;
                self.typecheck_stmt(body);
                self.loop_depth -= 1;
            }
            Stmt::Return { value } => {
                let expected_ty = self.current_return_type.last().copied().unwrap_or_else(|| self.ctx.types.intern(SemanticType::Void));
                if let Some(val) = value {
                    let val_ty = self.typecheck_expr(val);
                    if let Err(e) = self.unify(expected_ty, val_ty) {
                                    let span = self.get_expr_span_for_diag(val).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                    self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                }
                } else {
                    let void_ty = self.ctx.types.intern(SemanticType::Void);
                    if let Err(e) = self.unify(expected_ty, void_ty) {
                                    // stmt_id span is not easily available, fallback to 0
                                    self.ctx.diagnostics.push(Diagnostic::error(e).with_span(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0)));
                                }
                }
            }
            Stmt::Unsafe { body } => {
                let old = self.is_unsafe_context;
                self.is_unsafe_context = true;
                self.typecheck_stmt(body);
                self.is_unsafe_context = old;
            }
            Stmt::Break { .. } => {
                if self.loop_depth == 0 {
                    self.ctx.diagnostics.push(Diagnostic::error("`break` outside of a loop"));
                }
            }
            Stmt::Continue { .. } => {
                if self.loop_depth == 0 {
                    self.ctx.diagnostics.push(Diagnostic::error("`continue` outside of a loop"));
                }
            }
            other => {
                self.ctx.diagnostics.push(Diagnostic::error(format!("Unsupported or unrecognized statement construct in semantic phase: {:?}", other)));
            }
        }
    }

    fn resolve_type(&mut self, ty_id: mellis_ast::TypeId) -> SemanticTypeId {
        self.ctx.tables.ast_type_to_semantic.get(&ty_id).copied().unwrap_or_else(|| self.ctx.types.new_inference_var())
    }

    fn typecheck_expr(&mut self, expr_id: &mellis_ast::ExprId) -> SemanticTypeId {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        let ty_id = match expr {
            Expr::Literal(tok, _) => {
                // Determine type based on literal token type
                let kind = match tok.kind {
                    TokenKind::IntegerLiteral => SemanticType::Primitive(BuiltinType::I32), // Default to i32
                    TokenKind::FloatLiteral => SemanticType::Primitive(BuiltinType::F64), // Default to f64
                    TokenKind::StringLiteral => SemanticType::Primitive(BuiltinType::String),
                    TokenKind::CharLiteral => SemanticType::Primitive(BuiltinType::Char),
                    TokenKind::KwTrue | TokenKind::KwFalse => SemanticType::Primitive(BuiltinType::Bool),
                    _ => {
                        self.ctx.diagnostics.push(Diagnostic::error(format!("Unrecognized literal token '{:?}'", tok.kind)).with_span(tok.span));
                        SemanticType::Error
                    },
                };
                self.ctx.types.intern(kind)
            }
            Expr::Identifier { generic_args, .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {
                    let mut base_ty = None;
                    if let Some(ty) = self.ctx.tables.symbol_types.get(sym_id) {
                        base_ty = Some(*ty);
                    } else {
                        let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                        if let crate::SymbolKind::EnumVariant(_) = symbol.kind {
                            if let Some(decl_id) = symbol.decl_id {
                                if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {
                                    if let Some(ty) = self.ctx.tables.symbol_types.get(enum_sym_id) {
                                        base_ty = Some(*ty);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(ty) = base_ty {
                        if generic_args.is_empty() {
                            ty
                        } else {
                            // Substitute generic arguments
                            let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                            let decl_id_opt = symbol.decl_id.or_else(|| {
                                if let crate::SymbolKind::EnumVariant(_) = symbol.kind {
                                    symbol.decl_id
                                } else {
                                    None
                                }
                            });
                            
                            if let Some(decl_id) = decl_id_opt {
                                let mut concrete_args = Vec::new();
                                for arg in generic_args {
                                    concrete_args.push(self.lower_type(*arg));
                                }
                                let mut subst = crate::ty::Substitution::new();
                                for (idx, _) in generic_args.iter().enumerate() {
                                    if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(decl_id, idx)) {
                                        subst.insert(*gp_sym, concrete_args[idx]);
                                    }
                                }
                                let resolved_ty = self.ctx.types.get(ty).clone();
                                match resolved_ty {
                                    SemanticType::Struct(s_sym, _, original_field_tys) => {
                                        let mut field_tys = Vec::new();
                                        for orig_ty in original_field_tys {
                                            field_tys.push(self.ctx.types.subst(orig_ty, &subst));
                                        }
                                        self.ctx.types.intern(SemanticType::Struct(s_sym, concrete_args, field_tys))
                                    }
                                    SemanticType::Enum(e_sym, _, original_var_tys) => {
                                        let mut variant_tys = Vec::new();
                                        for orig_ty in original_var_tys {
                                            variant_tys.push(self.ctx.types.subst(orig_ty, &subst));
                                        }
                                        self.ctx.types.intern(SemanticType::Enum(e_sym, concrete_args, variant_tys))
                                    }
                                    SemanticType::Function { params, return_type } => {
                                        let mut new_params = Vec::new();
                                        for p in params {
                                            new_params.push(self.ctx.types.subst(p, &subst));
                                        }
                                        let new_ret = self.ctx.types.subst(return_type, &subst);
                                        self.ctx.types.intern(SemanticType::Function { params: new_params, return_type: new_ret })
                                    }
                                    _ => self.ctx.types.subst(ty, &subst),
                                }
                            } else {
                                ty
                            }
                        }
                    } else {
                        self.ctx.types.new_inference_var()
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
                
                if matches!(l_sem_ty, SemanticType::Pointer(_, _)) {
                    use mellis_ast::expr::BinaryOp;
                    if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                        if !self.is_unsafe_context {
                            self.ctx.diagnostics.push(mellis_common::Diagnostic::error("Pointer arithmetic requires an unsafe block."));
                        }
                    }
                }
                
                // For simplified logic: require left and right to be same
                if let Err(e) = self.unify(l_ty, r_ty) {
                                    let span = self.get_expr_span_for_diag(expr_id).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                    self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                } 
                
                use mellis_ast::expr::BinaryOp;
                match op {
                    BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge | BinaryOp::LogicAnd | BinaryOp::LogicOr => {
                        self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool))
                    }
                    BinaryOp::Range | BinaryOp::RangeInc => {
                        self.ctx.types.intern(SemanticType::Range(l_ty))
                    }
                    _ => l_ty
                }
            }
            Expr::Call { callee, args, .. } => {
                let callee_expr = &self.arena.exprs[callee.0 as usize];
                // the string name check for drop was removed from here
                
                let callee_ty_id = self.typecheck_expr(callee);
                let mut ret_ty_id = self.ctx.types.new_inference_var();
                let callee_ty = self.ctx.types.get(callee_ty_id).clone();
                let is_dyn_call = self.ctx.tables.dyn_method_indices.contains_key(callee);
                let is_struct_method = if let Expr::Member { .. } = callee_expr { self.ctx.tables.expr_symbols.contains_key(callee) } else { false };
                let is_method_call = (is_dyn_call || is_struct_method) && if let SemanticType::Function { ref params, .. } = callee_ty { params.len() == args.len() + 1 } else { false };
                let mut subst = crate::ty::Substitution::new();
                
                if let Some(&callee_sym) = self.ctx.tables.expr_symbols.get(callee) {
                    if Some(callee_sym) == self.ctx.lang_items.get(crate::lang_item::LangItem::DropFn) {
                        let diag = Diagnostic::error("Explicit calls to drop() are forbidden. Values are dropped automatically at end of scope.".to_string());
                        let diag = if let Some(span) = self.get_expr_span_for_diag(callee) { diag.with_span(span) } else { diag };
                        self.ctx.diagnostics.push(diag);
                    }
                }
                
                let mut has_generics = false;
                let mut func_sym_opt = None;
                if let Expr::Identifier { .. } = callee_expr {
                    if let Some(&callee_sym) = self.ctx.tables.expr_symbols.get(callee) {
                        func_sym_opt = Some(callee_sym);
                        if let Some(&callee_decl_id) = self.ctx.tables.symbol_decls.get(&callee_sym) {
                            if let Decl::Function { generic_params: m_gps, .. } = &self.arena.decls[callee_decl_id.0 as usize] {
                                has_generics = !m_gps.is_empty();
                                for (gp_idx, _) in m_gps.iter().enumerate() {
                                    if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(callee_decl_id, gp_idx)) {
                                        subst.insert(*gp_sym, self.ctx.types.new_inference_var());
                                    }
                                }
                            }
                        }
                    }
                }

                if let SemanticType::Function { params, return_type } = callee_ty {
                    ret_ty_id = if has_generics { self.ctx.types.subst(return_type, &subst) } else { return_type };
                    let expected_params = if is_method_call {
                        &params[1..]
                    } else {
                        &params[..]
                    };
                    for (i, arg) in args.iter().enumerate() {
                        let arg_ty = self.typecheck_expr(&arg.value);
                        if let Some(&expected_p) = expected_params.get(i) {
                            let expected_p = if has_generics { self.ctx.types.subst(expected_p, &subst) } else { expected_p };
                            if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                if let Err(e) = self.unify(expected_p, arg_ty) {
                                    let span = self.get_expr_span_for_diag(&arg.value).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                    self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                }
                            }
                        }
                    }
                    if has_generics {
                        let span = self.get_expr_span_for_diag(expr_id).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                        self.check_bounds_for_call(func_sym_opt.unwrap(), &subst, span);
                        
                        let empty = crate::ty::Substitution::new();
                        let mut resolved_subst = crate::ty::Substitution::new();
                        for (sym, ty) in subst.map {
                            resolved_subst.insert(sym, self.ctx.types.subst(ty, &empty));
                        }
                        if !resolved_subst.map.is_empty() {
                            self.ctx.tables.expr_substs.insert(*expr_id, resolved_subst);
                        }
                    }
                } else if let SemanticType::Enum(enum_sym_id, enum_args, variants) = callee_ty {
                    if let Some(func_sym) = func_sym_opt {
                        if let crate::SymbolKind::EnumVariant(variant_idx) = self.ctx.symbol_table.get_symbol(func_sym).kind {
                            let mut enum_subst = crate::ty::Substitution::new();
                            let mut has_enum_generics = false;
                            let mut new_args = Vec::new();
                            
                            has_enum_generics = !enum_args.is_empty();
                            for arg_ty in &enum_args {
                                if let SemanticType::GenericParam(gp_sym) = self.ctx.types.get(*arg_ty).clone() {
                                    let ivar = self.ctx.types.new_inference_var();
                                    enum_subst.insert(gp_sym, ivar);
                                    new_args.push(ivar);
                                }
                            }


                            let mut variant_ty = variants[variant_idx as usize];
                            if has_enum_generics {
                                variant_ty = self.ctx.types.subst(variant_ty, &enum_subst);
                            }
                            
                            if let SemanticType::Tuple(field_tys) = self.ctx.types.get(variant_ty).clone() {
                                for (i, arg) in args.iter().enumerate() {
                                    let arg_ty = self.typecheck_expr(&arg.value);
                                    if let Some(&expected_p) = field_tys.get(i) {
                                        if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                            if let Err(e) = self.unify(expected_p, arg_ty) {
                                                let span = self.get_expr_span_for_diag(&arg.value).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                                self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                            }
                                        }
                                    }
                                }
                            } else if variant_ty != self.ctx.types.intern(SemanticType::Void) {
                                if let Some(arg) = args.first() {
                                    let arg_ty = self.typecheck_expr(&arg.value);
                                    if !self.try_coerce_dyn(arg.value, arg_ty, variant_ty) {
                                        if let Err(e) = self.unify(variant_ty, arg_ty) {
                                            let span = self.get_expr_span_for_diag(&arg.value).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                            self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                        }
                                    }
                                }
                            }
                            
                            ret_ty_id = if has_enum_generics {
                                let new_variants: Vec<_> = variants.iter().map(|&v| self.ctx.types.subst(v, &enum_subst)).collect();
                                self.ctx.types.intern(SemanticType::Enum(enum_sym_id, new_args, new_variants))
                            } else {
                                callee_ty_id
                            };
                            
                            if has_enum_generics {
                                let span = self.get_expr_span_for_diag(expr_id).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                self.check_bounds_for_call(enum_sym_id, &enum_subst, span);
                                
                                let empty = crate::ty::Substitution::new();
                                let mut resolved_subst = crate::ty::Substitution::new();
                                for (sym, ty) in enum_subst.map {
                                    resolved_subst.insert(sym, self.ctx.types.subst(ty, &empty));
                                }
                                if !resolved_subst.map.is_empty() {
                                    self.ctx.tables.expr_substs.insert(*expr_id, resolved_subst);
                                }
                            }
                        }
                    }
                } else {
                    for arg in args {
                        self.typecheck_expr(&arg.value);
                    }
                }

                ret_ty_id
            }
            Expr::Assign { lvalue, value, .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(lvalue).copied() {
                    for lambda_id in &self.active_lambdas {
                        if self.ctx.tables.expr_captures.get(lambda_id).map_or(false, |caps| caps.contains(&sym_id)) {
                            self.ctx.tables.closure_mutated_captures.entry(*lambda_id).or_default().insert(sym_id);
                        }
                    }
                }
                let l_ty = self.typecheck_expr(lvalue);
                let r_ty = self.typecheck_expr(value);
                self.enforce_mutability(lvalue);
                if !self.try_coerce_dyn(*value, r_ty, l_ty) {
                    if let Err(e) = self.unify(l_ty, r_ty) {
                                    let span = self.get_expr_span_for_diag(expr_id).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                    self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                }
                }
                self.ctx.types.intern(SemanticType::Void)
            }
            Expr::Member { object, member } => {
                let obj_ty_id = self.typecheck_expr(object);
                let obj_ty = self.ctx.types.get(obj_ty_id).clone();
                let member_name = &self.source[member.start as usize..member.end as usize];
                
                // 1. Check if obj_ty is DynTrait or pointer/ref to DynTrait
                let dyn_trait_sym = match &obj_ty {
                    SemanticType::DynTrait(sym) => Some(*sym),
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        if let SemanticType::DynTrait(sym) = self.ctx.types.get(*inner) {
                            Some(*sym)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(trait_sym) = dyn_trait_sym {
                    if let Some(method_syms) = self.ctx.tables.trait_methods.get(&trait_sym) {
                        for (idx, &m_sym) in method_syms.iter().enumerate() {
                            let sym = self.ctx.symbol_table.get_symbol(m_sym);
                            if sym.name == member_name {
                                self.ctx.tables.dyn_method_indices.insert(*expr_id, idx as u32);
                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                    self.ctx.tables.expr_types.insert(*expr_id, m_ty);
                                    return m_ty;
                                }
                            }
                        }
                    }
                    self.ctx.diagnostics.push(Diagnostic::error(format!("Trait does not contain method '{}'", member_name)).with_span(*member));
                    let infer = self.ctx.types.new_inference_var();
                    self.ctx.tables.expr_types.insert(*expr_id, infer);
                    return infer;
                }
                
                let peeled_ty = match &obj_ty {
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        self.ctx.types.get(*inner).clone()
                    }
                    _ => obj_ty.clone(),
                };
                
                if let SemanticType::Struct(sym_id, _, field_tys) = peeled_ty {
                    if let Some(decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                        if let Decl::Struct { fields, .. } = &self.arena.decls[decl_id.0 as usize] {
                            if let Some((index, _)) = fields.iter().enumerate().find(|(_, field)| {
                                &self.source[field.name.start as usize..field.name.end as usize] == member_name
                            }) {
                                self.ctx.tables.expr_member_indices.insert(*expr_id, index as u32);
                                let f_ty = field_tys[index];
                                self.ctx.tables.expr_types.insert(*expr_id, f_ty);
                                return f_ty;
                            }
                        }
                    }

                    // Look up methods on this struct (from impl blocks)
                    for (impl_key, impl_decl_ids) in &self.ctx.tables.trait_impls {
                        for &impl_decl_id in impl_decl_ids {
                        if impl_key.self_type_def == sym_id {
                            if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                for &m_id in methods {
                                    if let Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {
                                        let m_name = &self.source[name.start as usize..name.end as usize];
                                        if m_name == member_name {
                                            if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {
                                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                                    self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);
                                                    self.ctx.tables.expr_types.insert(*expr_id, m_ty);
                                                    return m_ty;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        }
                    }

                    self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown field or method '{}'", member_name)).with_span(*member));
                    let infer = self.ctx.types.new_inference_var();
                    self.ctx.tables.expr_types.insert(*expr_id, infer);
                    return infer;
                }
                self.ctx.diagnostics.push(Diagnostic::error("Member access requires a struct or trait object").with_span(*member));
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
                let base_ty = self.ctx.tables.symbol_types.get(&symbol).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());
                let SemanticType::Struct(sym_id, _, original_field_tys) = self.ctx.types.get(base_ty).clone() else {
                    self.ctx.diagnostics.push(Diagnostic::error(format!("'{}' is not a struct", name)).with_span(*name_span));
                    return self.ctx.types.new_inference_var();
                };
                let Some(decl_id) = self.ctx.tables.symbol_decls.get(&symbol).copied() else {
                    return base_ty;
                };
                let Decl::Struct { fields: declared_fields, generic_params, .. } = &self.arena.decls[decl_id.0 as usize] else {
                    return base_ty;
                };

                let mut concrete_args = Vec::new();
                if let Expr::StructInit { generic_args, .. } = &self.arena.exprs[expr_id.0 as usize] {
                    for arg in generic_args {
                        concrete_args.push(self.lower_type(*arg));
                    }
                }
                
                let expected_args = generic_params.len();
                if concrete_args.len() < expected_args {
                    for _ in concrete_args.len()..expected_args {
                        concrete_args.push(self.ctx.types.new_inference_var());
                    }
                }

                let mut subst = crate::ty::Substitution::new();
                for (idx, _) in generic_params.iter().enumerate() {
                    if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(decl_id, idx)) {
                        if idx < concrete_args.len() {
                            subst.insert(*gp_sym, concrete_args[idx]);
                        }
                    }
                }
                
                if !generic_params.is_empty() {
                    let mut span = mellis_common::ids::Span::new(mellis_common::ids::FileId(0), 0, 0);
                    if let Some(first) = path.first() { span = *first; }
                    self.check_bounds_for_call(sym_id, &subst, span);
                }

                let mut field_tys = Vec::new();
                for orig_ty in original_field_tys {
                    field_tys.push(self.ctx.types.subst(orig_ty, &subst));
                }

                let struct_ty = self.ctx.types.intern(SemanticType::Struct(sym_id, concrete_args, field_tys.clone()));

                let mut init_indices = Vec::with_capacity(fields.len());
                for field in fields {
                    let field_name = &self.source[field.name.start as usize..field.name.end as usize];
                    if let Some((index, _)) = declared_fields.iter().enumerate().find(|(_, declared)| {
                        &self.source[declared.name.start as usize..declared.name.end as usize] == field_name
                    }) {
                        let value_ty = self.typecheck_expr(&field.value);
                        if self.unify(field_tys[index], value_ty).is_err() {
                            self.ctx.diagnostics.push(Diagnostic::error(format!("Type mismatch for field '{}'", field_name)).with_span(field.name));
                        }
                        init_indices.push(index as u32);
                    } else {
                        self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown field '{}' for struct '{}'", field_name, name)).with_span(field.name));
                        self.typecheck_expr(&field.value);
                        init_indices.push(u32::MAX); // Error recovery
                    }
                }
                self.ctx.tables.expr_struct_init_indices.insert(*expr_id, init_indices);
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
                let member_name = &self.source[method_name.start as usize..method_name.end as usize];
                
                // 1. Check if obj_ty is DynTrait or pointer/ref to DynTrait
                let dyn_trait_sym = match &obj_ty {
                    SemanticType::DynTrait(sym) => Some(*sym),
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        if let SemanticType::DynTrait(sym) = self.ctx.types.get(*inner) {
                            Some(*sym)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(trait_sym) = dyn_trait_sym {
                    if let Some(method_syms) = self.ctx.tables.trait_methods.get(&trait_sym) {
                        for (idx, &m_sym) in method_syms.iter().enumerate() {
                            let sym = self.ctx.symbol_table.get_symbol(m_sym);
                            if sym.name == member_name {
                                self.ctx.tables.dyn_method_indices.insert(*expr_id, idx as u32);
                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                    if let SemanticType::Function { params, return_type } = self.ctx.types.get(m_ty).clone() {
                                        let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };
                                        for (i, arg) in args.iter().enumerate() {
                                            let arg_ty = self.typecheck_expr(&arg.value);
                                            if let Some(&expected_p) = expected_params.get(i) {
                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                                    if let Err(e) = self.unify(expected_p, arg_ty) {
                                    let span = self.get_expr_span_for_diag(&arg.value).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                    self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                }
                                                }
                                            }
                                        }
                                        return return_type;
                                    }
                                }
                            }
                        }
                    }
                }

                // 2. Struct method lookup
                let peeled_ty = match &obj_ty {
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        self.ctx.types.get(*inner).clone()
                    }
                    _ => obj_ty.clone(),
                };

                if let SemanticType::Struct(sym_id, _, _) = peeled_ty {
                    for (impl_key, impl_decl_ids) in &self.ctx.tables.trait_impls {
                        for &impl_decl_id in impl_decl_ids {
                        if impl_key.self_type_def == sym_id {
                            if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                for &m_id in methods {
                                    if let Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {
                                        let m_name = &self.source[name.start as usize..name.end as usize];
                                        if m_name == member_name {
                                            if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {
                                                self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);
                                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                                    let semantic_ty = self.ctx.types.get(m_ty).clone();                                                    if let SemanticType::Function { params, return_type } = semantic_ty {
                                                        let mut subst = crate::ty::Substitution::new();
                                                        let mut m_has_generics = false;
                                                        if let Some(&method_decl_id) = self.ctx.tables.symbol_decls.get(&m_sym) {
                                                            if let Decl::Function { generic_params: m_gps, .. } = &self.arena.decls[method_decl_id.0 as usize] {
                                                                m_has_generics = !m_gps.is_empty();
                                                                for (gp_idx, _) in m_gps.iter().enumerate() {
                                                                    if let Some(gp_sym) = self.ctx.tables.generic_param_symbols.get(&(method_decl_id, gp_idx)) {
                                                                        subst.insert(*gp_sym, self.ctx.types.new_inference_var());
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        let return_type = if m_has_generics { self.ctx.types.subst(return_type, &subst) } else { return_type };
                                                        let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };
                                                        for (i, arg) in args.iter().enumerate() {
                                                            let arg_ty = self.typecheck_expr(&arg.value);
                                                            if let Some(&expected_p) = expected_params.get(i) {
                                                                let expected_p = if m_has_generics { self.ctx.types.subst(expected_p, &subst) } else { expected_p };
                                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                                                    if let Err(e) = self.unify(expected_p, arg_ty) {
                                                                        let span = self.get_expr_span_for_diag(&arg.value).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                                                                        self.ctx.diagnostics.push(Diagnostic::error(e).with_span(span));
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        if m_has_generics {
                                                            self.check_bounds_for_call(m_sym, &subst, *method_name);
                                                            
                                                            let empty = crate::ty::Substitution::new();
                                                            let mut resolved_subst = crate::ty::Substitution::new();
                                                            for (sym, ty) in subst.map {
                                                                resolved_subst.insert(sym, self.ctx.types.subst(ty, &empty));
                                                            }
                                                            if !resolved_subst.map.is_empty() {
                                                                self.ctx.tables.expr_substs.insert(*expr_id, resolved_subst);
                                                            }
                                                        }

                                                        return return_type;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        }
                    }
                }

                for arg in args {
                    self.typecheck_expr(&arg.value);
                }

                self.ctx.types.new_inference_var()
            }
            Expr::Sizeof { target_type } => {
                self.lower_type(*target_type);
                self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::I32))
            }
            Expr::Alignof { target_type } => {
                self.lower_type(*target_type);
                self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::I32))
            }
            Expr::Cast { expr: e, target_type } => {
                self.typecheck_expr(e);
                self.lower_type(*target_type)
            }
            Expr::Match { subject, arms, match_span } => {
                let subject_ty_id = self.typecheck_expr(subject);
                let subject_ty = self.ctx.types.get(subject_ty_id).clone();
                let mut result_ty = self.ctx.types.new_inference_var();
                
                let mut covered_variants = std::collections::HashSet::new();
                let mut covered_bools = std::collections::HashSet::new();
                let mut has_wildcard = false;

                for (i, arm) in arms.iter().enumerate() {
                    self.typecheck_pattern(&arm.pattern, subject_ty_id);
                    
                    match &self.arena.pats[arm.pattern.0 as usize] {
                        mellis_ast::Pattern::Wildcard => {
                            has_wildcard = true;
                        }
                        mellis_ast::Pattern::Identifier { .. } => {
                            if let Some(sym_id) = self.ctx.tables.pat_symbols.get(&arm.pattern) {
                                let sym = self.ctx.symbol_table.get_symbol(*sym_id);
                                if matches!(sym.kind, crate::symbol::SymbolKind::EnumVariant(_)) {
                                    covered_variants.insert(*sym_id);
                                } else {
                                    has_wildcard = true;
                                }
                            } else {
                                has_wildcard = true;
                            }
                        }
                        mellis_ast::Pattern::Enum { .. } => {
                            if let Some(sym_id) = self.ctx.tables.pat_symbols.get(&arm.pattern) {
                                covered_variants.insert(*sym_id);
                            }
                        }
                        mellis_ast::Pattern::Literal(tok) => {
                            if tok.kind == mellis_lexer::TokenKind::KwTrue {
                                covered_bools.insert(true);
                            } else if tok.kind == mellis_lexer::TokenKind::KwFalse {
                                covered_bools.insert(false);
                            }
                        }
                        _ => {}
                    }
                    
                    // Typecheck arm body
                    self.typecheck_stmt(&arm.body);
                    // Extract arm result type
                    let mut arm_ty = self.ctx.types.intern(SemanticType::Void);
                    match &self.arena.stmts[arm.body.0 as usize] {
                        Stmt::Block { tail_expr: Some(tail), .. } => {
                            if let Some(ty) = self.ctx.tables.expr_types.get(tail).copied() { arm_ty = ty; }
                        }
                        Stmt::Expr { expr, has_semicolon: false } => {
                            if let Some(ty) = self.ctx.tables.expr_types.get(expr).copied() { arm_ty = ty; }
                        }
                        _ => {}
                    }
                    
                    if let Err(err) = self.unify(result_ty, arm_ty) {
                        self.ctx.diagnostics.push(mellis_common::diagnostic::Diagnostic::error(err).with_span(*match_span));
                    }
                }
                
                // Exhaustiveness check
                if !has_wildcard {
                    match subject_ty {
                        SemanticType::Primitive(crate::ty::BuiltinType::Bool) => {
                            if !covered_bools.contains(&true) || !covered_bools.contains(&false) {
                                self.ctx.diagnostics.push(
                                    mellis_common::diagnostic::Diagnostic::error("Match is not exhaustive. Missing boolean values.")
                                        .with_span(*match_span)
                                );
                            }
                        }
                        SemanticType::Enum(sym_id, _, _) => {
                            let decl_id = self.ctx.symbol_table.get_symbol(sym_id).decl_id;
                            if let Some(decl_id) = decl_id {
                                if let mellis_ast::Decl::Enum { variants, .. } = &self.arena.decls[decl_id.0 as usize] {
                                    if covered_variants.len() < variants.len() {
                                        self.ctx.diagnostics.push(
                                            mellis_common::diagnostic::Diagnostic::error(format!("Match is not exhaustive. Covered {}/{} enum variants.", covered_variants.len(), variants.len()))
                                                .with_span(*match_span)
                                        );
                                    }
                                }
                            }
                        }
                        SemanticType::InferenceVar(_) | SemanticType::Error => {}
                        _ => {
                            self.ctx.diagnostics.push(
                                mellis_common::diagnostic::Diagnostic::error("Match is not exhaustive. A wildcard pattern `_` or variable binding is required for this type.")
                                    .with_span(*match_span)
                            );
                        }
                    }
                }
                
                result_ty
            }
            Expr::Lambda { body, params, return_type, is_move } => {
                let mut param_tys = Vec::new();
                for param_id in params {
                    if let mellis_ast::Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {
                        let ty_id = if let Some(param_ty) = ty {
                            self.lower_type(*param_ty)
                        } else {
                            self.ctx.types.new_inference_var()
                        };
                        if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(param_id) {
                            self.ctx.tables.symbol_types.insert(sym_id, ty_id);
                        }
                        param_tys.push(ty_id);
                    } else {
                        param_tys.push(self.ctx.types.new_inference_var());
                    }
                }
                let ret_ty_id = if let Some(ret_ty) = return_type {
                    self.lower_type(*ret_ty)
                } else {
                    self.ctx.types.new_inference_var()
                };
                
                self.active_lambdas.push(*expr_id);
                self.current_return_type.push(ret_ty_id);
                self.typecheck_stmt(body);
                self.current_return_type.pop();
                self.active_lambdas.pop();
                if let Some(body_ty) = self.infer_stmt_value_type(body) {
                    if let Err(_) = self.unify(ret_ty_id, body_ty) {
                        self.ctx.diagnostics.push(Diagnostic::error("Lambda return type mismatch"));
                    }
                }
                let ret_ty_id = self.ctx.types.resolve_inference(ret_ty_id);
                
                let mut env_tys = Vec::new();
                let mut capture_bindings = Vec::new();
                if let Some(captures) = self.ctx.tables.expr_captures.get(expr_id).cloned() {
                    for (field, sym_id) in captures.into_iter().enumerate() {
                        let sym_ty = self.ctx.tables.symbol_types.get(&sym_id).copied().unwrap_or_else(|| self.ctx.types.intern(SemanticType::Error));
                        let is_mutated = self.ctx.tables.closure_mutated_captures.get(expr_id).map_or(false, |mutated| mutated.contains(&sym_id));
                        let mode = if *is_move {
                            crate::semantic_tables::CaptureMode::Move
                        } else if is_mutated {
                            crate::semantic_tables::CaptureMode::MutableBorrow
                        } else {
                            crate::semantic_tables::CaptureMode::SharedBorrow
                        };
                        
                        let env_ty = match mode {
                            crate::semantic_tables::CaptureMode::MutableBorrow => {
                                self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Mutable, sym_ty))
                            }
                            crate::semantic_tables::CaptureMode::SharedBorrow => {
                                self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Immutable, sym_ty))
                            }
                            crate::semantic_tables::CaptureMode::Move => sym_ty,
                        };
                        env_tys.push(env_ty);
                        let is_mutated = self.ctx.tables.closure_mutated_captures.get(expr_id).map_or(false, |mutated| mutated.contains(&sym_id));
                        let is_mutable = self.ctx.tables.symbol_decls.get(&sym_id).and_then(|decl_id| match &self.arena.decls[decl_id.0 as usize] {
                            Decl::Var { is_mutable, .. } => Some(*is_mutable),
                            Decl::Param { .. } => Some(true),
                            _ => None,
                        }).unwrap_or(false);
                        if is_mutated && !is_mutable {
                            self.ctx.diagnostics.push(Diagnostic::error("Cannot mutably capture immutable variable"));
                        }
                        capture_bindings.push(crate::semantic_tables::CaptureBinding {
                            symbol: sym_id,
                            mode,
                            env_field: field as u32,
                            ty: sym_ty,
                            env_ty,
                        });
                    }
                }
                self.ctx.tables.closure_capture_bindings.insert(*expr_id, capture_bindings);
                let env_tuple_ty = self.ctx.types.intern(SemanticType::Tuple(env_tys));
                self.ctx.tables.closure_env_types.insert(*expr_id, env_tuple_ty);
                let env_ptr_ty = self.ctx.types.intern(SemanticType::Pointer(
                    crate::ty::Mutability::Immutable,
                    env_tuple_ty,
                ));
                self.ctx.tables.closure_env_ptr_types.insert(*expr_id, env_ptr_ty);
                
                self.ctx.types.intern(SemanticType::Closure(
                    *expr_id,
                    param_tys,
                    ret_ty_id,
                ))
            }
            Expr::Try { expr: e, .. } => {
                let inner_ty_id = self.typecheck_expr(e);
                
                let try_sym = self.ctx.lang_items.get(crate::lang_item::LangItem::Try);
                let from_residual_sym = self.ctx.lang_items.get(crate::lang_item::LangItem::FromResidual);
                
                if try_sym.is_none() || from_residual_sym.is_none() {
                    let mut diag = Diagnostic::error("Missing language item: `Try` or `FromResidual`");
                    if let Some(span) = self.get_expr_span_for_diag(expr_id) {
                        diag.span = Some(span);
                    }
                    self.ctx.diagnostics.push(diag);
                    return self.ctx.types.intern(SemanticType::Error);
                }
                
                let try_sym = try_sym.unwrap();
                let from_residual_sym = from_residual_sym.unwrap();
                
                let mut output_ty = self.ctx.types.new_inference_var();
                let mut residual_ty = self.ctx.types.new_inference_var();
                let mut found_try_impl = false;
                
                let inner_ty = self.ctx.types.get(inner_ty_id).clone();
                let mut inner_def_sym = None;
                if let SemanticType::Enum(sym, _, _) | SemanticType::Struct(sym, _, _) = &inner_ty {
                    inner_def_sym = Some(*sym);
                }
                
                let try_branch_name = self.ctx.lang_items.get(crate::lang_item::LangItem::TryBranch)
                    .map(|s| self.ctx.symbol_table.get_symbol(s).name.as_str().to_string());
                    
                println!("DEBUG: try_branch_name = {:?}", try_branch_name);
                
                if let Some(def_sym) = inner_def_sym {
                    let key = crate::semantic_tables::ImplKey {
                        trait_id: Some(try_sym),
                        self_type_def: def_sym,
                    };
                    if let Some(method_syms) = self.ctx.tables.impl_methods.get(&key) {
                        for &m_sym in method_syms {
                            let m_sym_name = self.ctx.symbol_table.get_symbol(m_sym).name.as_str();
                            println!("DEBUG: checking impl method = {:?}", m_sym_name);
                            if Some(m_sym_name) == try_branch_name.as_deref() {
                                let m_decl = self.ctx.tables.symbol_decls.get(&m_sym).copied().unwrap();
                                self.ctx.tables.try_branch_methods.insert(*expr_id, m_decl);
                                
                                if let Some(SemanticType::Function { return_type: ret_ty, .. }) = self.ctx.tables.symbol_types.get(&m_sym).map(|&t| self.ctx.types.get(t).clone()) {
                                    if let SemanticType::Enum(cf_sym, args, _) = self.ctx.types.get(ret_ty).clone() {
                                        if args.len() == 2 {
                                            residual_ty = args[0];
                                            output_ty = args[1];
                                            found_try_impl = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                if !found_try_impl {
                    let mut diag = Diagnostic::error("The `?` operator can only be applied to types that implement `Try`");
                    if let Some(span) = self.get_expr_span_for_diag(e) {
                        diag.span = Some(span);
                    }
                    self.ctx.diagnostics.push(diag);
                }
                
                let current_ret_ty = self.current_return_type.last().copied().unwrap_or_else(|| self.ctx.types.intern(SemanticType::Void));
                let mut found_from_residual_impl = false;
                
                let mut ret_def_sym = None;
                if let SemanticType::Enum(sym, _, _) | SemanticType::Struct(sym, _, _) = &self.ctx.types.get(current_ret_ty).clone() {
                    ret_def_sym = Some(*sym);
                }
                
                let from_residual_name = self.ctx.lang_items.get(crate::lang_item::LangItem::FromResidualFn)
                    .map(|s| self.ctx.symbol_table.get_symbol(s).name.as_str().to_string());
                    
                if let Some(def_sym) = ret_def_sym {
                    let key = crate::semantic_tables::ImplKey {
                        trait_id: Some(from_residual_sym),
                        self_type_def: def_sym,
                    };
                    
                    if let Some(method_syms) = self.ctx.tables.impl_methods.get(&key) {
                        for &m_sym in method_syms {
                            let m_sym_name = self.ctx.symbol_table.get_symbol(m_sym).name.as_str();
                            if Some(m_sym_name) == from_residual_name.as_deref() {
                                let m_decl = self.ctx.tables.symbol_decls.get(&m_sym).copied().unwrap();
                                self.ctx.tables.try_from_residual_methods.insert(*expr_id, m_decl);
                                found_from_residual_impl = true;
                                break;
                            }
                        }
                    }
                }
                
                if !found_from_residual_impl {
                    let mut diag = Diagnostic::error("The `?` operator cannot be used in a function that returns this type, as it does not implement `FromResidual` for the residual type");
                    if let Some(span) = self.get_expr_span_for_diag(expr_id) {
                        diag.span = Some(span);
                    }
                    self.ctx.diagnostics.push(diag);
                }
                
                output_ty
            }
            Expr::Await { expr } => {
                if self.current_async_fn.is_none() {
                    let mut diag = Diagnostic::error("`await` is only allowed inside `async` functions");
                    if let Some(span) = self.get_expr_span_for_diag(expr) {
                        diag.span = Some(span);
                    }
                    self.ctx.diagnostics.push(diag);
                }
                let inner_ty_id = self.typecheck_expr(expr);
                let inner_ty = self.ctx.types.get(inner_ty_id).clone();
                match inner_ty {
                    SemanticType::Future(out_ty) => out_ty,
                    SemanticType::InferenceVar(var) => {
                        let out_ty = self.ctx.types.new_inference_var();
                        let fut_ty = self.ctx.types.intern(SemanticType::Future(out_ty));
                        self.ctx.types.inference_bindings.insert(var, fut_ty);
                        out_ty
                    }
                    SemanticType::Error => self.ctx.types.intern(SemanticType::Error),
                    _ => {
                        let mut diag = Diagnostic::error("Cannot await a non-future type");
                        if let Some(span) = self.get_expr_span_for_diag(expr) {
                            diag.span = Some(span);
                        }
                        self.ctx.diagnostics.push(diag);
                        self.ctx.types.intern(SemanticType::Error)
                    }
                }
            }
            Expr::Unary { op, operand } => {
                let inner_ty = self.typecheck_expr(operand);

                use mellis_ast::expr::UnaryOp;
                match op {
                    UnaryOp::Ref => self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Immutable, inner_ty)),
                    UnaryOp::RefMut => {
                        self.enforce_mutability(operand);
                        self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Mutable, inner_ty))
                    },
                    UnaryOp::Deref | UnaryOp::DerefMut => {
                        match self.ctx.types.get(inner_ty) {
                            SemanticType::Pointer(_, pointee) | SemanticType::Reference(_, _, pointee) => *pointee,
                            _ => inner_ty,
                        }
                    }
                    _ => inner_ty,
                }
            }
            Expr::Comptime { body } => {
                self.typecheck_stmt(body);
                let ty = self.infer_stmt_value_type(body).unwrap_or_else(|| self.ctx.types.intern(SemanticType::Void));
                match self.eval_comptime_stmt(*body) {
                    Ok(v) => { self.ctx.comptime_values.insert(*expr_id, v); }
                    Err(e) => {
                        let span = self.get_expr_span_for_diag(expr_id).unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                        self.ctx.diagnostics.push(Diagnostic::error(format!("cannot evaluate comptime block: {}", e)).with_span(span));
                    }
                }
                ty
            }
            other => {
                let mut diag = Diagnostic::error(format!("Unsupported or unrecognized expression construct in semantic phase: {:?}", other));
                if let Some(span) = self.get_expr_span_for_diag(expr_id) {
                    diag.span = Some(span);
                }
                self.ctx.diagnostics.push(diag);
                self.ctx.types.intern(SemanticType::Error)
            }
        };
        
        self.ctx.tables.expr_types.insert(*expr_id, ty_id);
        ty_id
    }

    fn get_expr_span_for_diag(&self, expr_id: &mellis_ast::ExprId) -> Option<mellis_common::Span> {
        let expr = self.arena.exprs.get(expr_id.0 as usize)?;
        match expr {
            Expr::Literal(tok, _) => Some(tok.span),
            Expr::Identifier { segments, .. } => segments.first().copied(),
            Expr::Call { callee, .. } => self.get_expr_span_for_diag(callee),
            Expr::MethodCall { method_name, .. } => Some(*method_name),
            Expr::Await { expr } => self.get_expr_span_for_diag(expr),
            Expr::Try { try_span, .. } => Some(*try_span),
            _ => None,
        }
    }

    fn enforce_mutability(&mut self, expr_id: &mellis_ast::ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        if let mellis_ast::Expr::Identifier { segments, .. } = expr {
            if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {
                let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                if matches!(symbol.kind, crate::symbol::SymbolKind::Constant) {
                    let mut diag = mellis_common::diagnostic::Diagnostic::error("Cannot mutate immutable variable");
                    if let Some(&span) = segments.first() {
                        diag.span = Some(span);
                    }
                    self.ctx.diagnostics.push(diag);
                }
            }
        }
    }
}
