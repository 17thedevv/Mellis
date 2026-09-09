use mellis_ast::{AstArena, Item, Stmt, Expr, Decl, DeclId, ExprId, StmtId};
use crate::{SemanticContext, ty::{SemanticTypeId, SemanticType, Substitution}};
use mellis_common::ids::SymbolId;
use std::collections::{HashSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoInstance {
    pub decl_id: DeclId,
    pub subst: Vec<(SymbolId, SemanticTypeId)>,
    pub closure_id: Option<mellis_ast::ExprId>,
}

#[derive(Debug, Clone)]
pub struct InstantiatedFunction {
    pub instance: MonoInstance,
    pub expr_types: HashMap<ExprId, SemanticTypeId>,
    pub symbol_types: HashMap<SymbolId, SemanticTypeId>,
    pub pat_types: HashMap<mellis_ast::PatId, SemanticTypeId>,
    pub mono_calls: HashMap<ExprId, MonoInstance>,
    pub mono_for_loops: HashMap<StmtId, MonoInstance>,
    pub closure_capture_bindings: Vec<crate::semantic_tables::CaptureBinding>,
    pub closure_env_type: Option<SemanticTypeId>,
    pub closure_env_ptr_type: Option<SemanticTypeId>,
}

pub struct MonoCollector<'a> {
    ctx: &'a mut SemanticContext,
    arena: &'a AstArena,
    
    pub worklist: Vec<MonoInstance>,
    pub instantiated: HashMap<MonoInstance, InstantiatedFunction>,
    
    // Temporary state
    current_instance: Option<MonoInstance>,
    current_expr_types: HashMap<ExprId, SemanticTypeId>,
    current_symbol_types: HashMap<SymbolId, SemanticTypeId>,
    current_pat_types: HashMap<mellis_ast::PatId, SemanticTypeId>,
    current_mono_calls: HashMap<ExprId, MonoInstance>,
    current_mono_for_loops: HashMap<StmtId, MonoInstance>,
    current_subst: Substitution,
}

impl<'a> MonoCollector<'a> {
    pub fn new(ctx: &'a mut SemanticContext, arena: &'a AstArena) -> Self {
        Self {
            ctx,
            arena,
            worklist: Vec::new(),
            instantiated: HashMap::new(),
            current_instance: None,
            current_expr_types: HashMap::new(),
            current_symbol_types: HashMap::new(),
            current_pat_types: HashMap::new(),
            current_mono_calls: HashMap::new(),
            current_mono_for_loops: HashMap::new(),
            current_subst: Substitution::new(),
        }
    }

    fn collect_function_roots(&mut self, items: &[Item]) {
        for item in items {
            if let Item::Decl(decl_id) = item {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { generic_params, .. } => {
                        if generic_params.is_empty() {
                            self.worklist.push(MonoInstance {
                                decl_id: *decl_id,
                                subst: vec![],
                                closure_id: None,
                            });
                        }
                    }
                    Decl::Module { items: inner_decls, .. } => {
                        let inner_items: Vec<Item> = inner_decls.iter().map(|&d| Item::Decl(d)).collect();
                        self.collect_function_roots(&inner_items);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn run(&mut self, items: &[Item]) {
        self.collect_function_roots(items);

        // Also root methods for all concrete-to-dyn coercions so vtable pointers are generated
        for coercion in self.ctx.tables.coercions.values() {
            if let crate::coercion::CoercionKind::ConcreteToDyn { trait_sym, concrete_sym } = coercion {
                let key = crate::semantic_tables::ImplKey {
                    trait_id: Some(*trait_sym),
                    self_type_def: *concrete_sym,
                };
                if let Some(impl_decls) = self.ctx.tables.trait_impls.get(&key) {
                    for impl_decl_id in impl_decls {
                        if let Decl::Impl { methods, generic_params, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                            if generic_params.is_empty() {
                                for m_id in methods {
                                    self.worklist.push(MonoInstance {
                                        decl_id: *m_id,
                                        subst: vec![],
                                        closure_id: None,
                                    });
                                }
                            }
                        }
                    }
                }
                if let Some(drop_sym) = self.ctx.lang_items.get(crate::lang_item::LangItem::Drop) {
                    let drop_key = crate::semantic_tables::ImplKey {
                        trait_id: Some(drop_sym),
                        self_type_def: *concrete_sym,
                    };
                    if let Some(drop_impls) = self.ctx.tables.trait_impls.get(&drop_key) {
                        for impl_decl_id in drop_impls {
                            if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                for m_id in methods {
                                    self.worklist.push(MonoInstance {
                                        decl_id: *m_id,
                                        subst: vec![],
                                        closure_id: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        self.process_worklist();
    }

    pub fn run_on_expr(&mut self, expr_id: ExprId) {
        let dummy_instance = MonoInstance {
            decl_id: DeclId(0), // Dummy for local visit
            subst: vec![],
            closure_id: None,
        };
        self.current_instance = Some(dummy_instance);
        self.current_expr_types.clear();
        self.current_symbol_types.clear();
        self.current_pat_types.clear();
        self.current_mono_calls.clear();
        self.current_mono_for_loops.clear();
        self.current_subst.map.clear();
        
        self.visit_expr(&expr_id);
        
        self.process_worklist();
    }

    pub fn run_on_stmt(&mut self, stmt_id: StmtId) {
        let dummy_instance = MonoInstance {
            decl_id: DeclId(0), // Dummy for local visit
            subst: vec![],
            closure_id: None,
        };
        self.current_instance = Some(dummy_instance);
        self.current_expr_types.clear();
        self.current_symbol_types.clear();
        self.current_pat_types.clear();
        self.current_mono_calls.clear();
        self.current_mono_for_loops.clear();
        self.current_subst.map.clear();
        
        self.visit_stmt(&stmt_id);
        
        self.process_worklist();
    }

    fn process_worklist(&mut self) {
        while let Some(instance) = self.worklist.pop() {
            if self.instantiated.contains_key(&instance) {
                continue;
            }

            self.current_instance = Some(instance.clone());
            self.current_expr_types.clear();
            self.current_symbol_types.clear();
            self.current_pat_types.clear();
            self.current_mono_calls.clear();
            self.current_mono_for_loops.clear();
            self.current_subst.map.clear();

            for &(sym, ty) in &instance.subst {
                self.current_subst.insert(sym, ty);
            }

            if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(&instance.decl_id) {
                if let Some(&ty) = self.ctx.tables.symbol_types.get(&sym_id) {
                    let sub_ty = self.substitute(ty);
                    self.current_symbol_types.insert(sym_id, sub_ty);
                }
            }

            if let Some(closure_id) = instance.closure_id {
                if let Expr::Lambda { body, params, .. } = &self.arena.exprs[closure_id.0 as usize] {
                    for param_id in params {
                        if let Some(&param_sym) = self.ctx.tables.decl_symbols.get(param_id) {
                            if let Some(&ty) = self.ctx.tables.symbol_types.get(&param_sym) {
                                let sub_ty = self.substitute(ty);
                                self.current_symbol_types.insert(param_sym, sub_ty);
                            }
                        }
                    }
                    self.visit_stmt(body);
                }
            } else {
                let decl = &self.arena.decls[instance.decl_id.0 as usize];
                if let Decl::Function { body: Some(body_stmt), params, .. } = decl {
                    for param_id in params {
                        if let Some(&param_sym) = self.ctx.tables.decl_symbols.get(param_id) {
                            if let Some(&ty) = self.ctx.tables.symbol_types.get(&param_sym) {
                                let sub_ty = self.substitute(ty);
                                self.current_symbol_types.insert(param_sym, sub_ty);
                            }
                        }
                    }
                    self.visit_stmt(body_stmt);
                }
            }

            let closure_bindings = instance.closure_id
                .map(|id| self.ctx.tables.expect_closure_capture_bindings(id))
                .unwrap_or_default();
            let closure_env_type = instance.closure_id
                .and_then(|id| self.ctx.tables.closure_env_types.get(&id).copied());
            let closure_env_ptr_type = instance.closure_id
                .and_then(|id| self.ctx.tables.closure_env_ptr_types.get(&id).copied());
            let closure_capture_bindings = closure_bindings.into_iter().map(|mut binding| {
                binding.ty = self.substitute(binding.ty);
                binding.env_ty = self.substitute(binding.env_ty);
                binding
            }).collect();
            
            // --- CONCRETIZATION BARRIER ---
            let mut is_concrete = true;

            let mut current_expr_types = std::mem::take(&mut self.current_expr_types);
            for (expr_id, ty) in current_expr_types.iter_mut() {
                let resolved = self.substitute(*ty);
                *ty = resolved;
                
                // Exempt functions and enum variants from being monomorphic as expressions
                // because they are instantiations of generics themselves (e.g., `Vec::new`).
                if let mellis_ast::Expr::Identifier { .. } | mellis_ast::Expr::Member { .. } = &self.arena.exprs[expr_id.0 as usize] {
                    if matches!(self.ctx.types.get(resolved), crate::ty::SemanticType::Function { .. } | crate::ty::SemanticType::Enum(_, _, _)) {
                        continue;
                    }
                }

                if !self.ctx.types.is_monomorphic(resolved) {
                    self.emit_monomorphization_barrier_error(resolved, Some(expr_id));
                    is_concrete = false;
                }
            }
            
            let mut current_symbol_types = std::mem::take(&mut self.current_symbol_types);
            for (sym_id, ty) in current_symbol_types.iter_mut() {
                let resolved = self.substitute(*ty);
                *ty = resolved;
                
                if matches!(self.ctx.types.get(resolved), crate::ty::SemanticType::Function { .. }) {
                    continue;
                }

                if !self.ctx.types.is_monomorphic(resolved) {
                    self.emit_monomorphization_barrier_error(resolved, None);
                    is_concrete = false;
                }
            }

            let mut current_pat_types = std::mem::take(&mut self.current_pat_types);
            for (_pat_id, ty) in current_pat_types.iter_mut() {
                let resolved = self.substitute(*ty);
                *ty = resolved;
                if !self.ctx.types.is_monomorphic(resolved) {
                    self.emit_monomorphization_barrier_error(resolved, None);
                    is_concrete = false;
                }
            }

            if !is_concrete {
                continue; // Barrier: do not insert unresolved mono unit
            }

            let instantiated_fn = InstantiatedFunction {
                instance: instance.clone(),
                expr_types: current_expr_types,
                symbol_types: current_symbol_types,
                pat_types: current_pat_types,
                mono_calls: std::mem::take(&mut self.current_mono_calls),
                mono_for_loops: std::mem::take(&mut self.current_mono_for_loops),
                closure_capture_bindings,
                closure_env_type: closure_env_type.map(|ty| self.substitute(ty)),
                closure_env_ptr_type: closure_env_ptr_type.map(|ty| self.substitute(ty)),
            };

            self.instantiated.insert(instance, instantiated_fn);
        }
    }

    fn normalize_projection(&mut self, self_ty: SemanticTypeId, trait_id: SymbolId, assoc_type: SymbolId) -> SemanticTypeId {
        let self_ty = self.ctx.types.resolve(self_ty);
        let sem_ty = self.ctx.types.get(self_ty).clone();
        let (nominal_sym, generic_args) = match &sem_ty {
            SemanticType::Struct(s, args, _) => (*s, args.clone()),
            SemanticType::Enum(e, args, _) => (*e, args.clone()),
            SemanticType::Reference(_, _, inner) | SemanticType::Pointer(_, inner) => {
                return self.normalize_projection(*inner, trait_id, assoc_type);
            }
            _ => {
                return self.ctx.types.intern(SemanticType::Error);
            }
        };

        let impl_key = crate::semantic_tables::ImplKey {
            trait_id: Some(trait_id),
            self_type_def: nominal_sym,
        };

        let decl_ids = match self.ctx.tables.trait_impls.get(&impl_key) {
            Some(ids) if !ids.is_empty() => ids.clone(),
            _ => {
                return self.ctx.types.intern(SemanticType::Error);
            }
        };

        if decl_ids.len() > 1 {
            return self.ctx.types.intern(SemanticType::Error);
        }

        let target_ty = match self.ctx.tables.impl_associated_types.get(&(impl_key.clone(), assoc_type)).copied() {
            Some(t) => t,
            None => return self.ctx.types.intern(SemanticType::Error),
        };

        let substituted_ty = if let Some(&impl_self_ty) = self.ctx.tables.impl_self_types.get(&impl_key) {
            let mut subst = crate::ty::Substitution::new();
            self.match_types(impl_self_ty, self_ty, &mut subst);
            self.ctx.types.subst(target_ty, &subst)
        } else {
            target_ty
        };

        self.normalize_type(substituted_ty)
    }

    fn match_types(&self, pattern_ty: SemanticTypeId, concrete_ty: SemanticTypeId, subst: &mut crate::ty::Substitution) {
        let pattern_ty = self.ctx.types.resolve(pattern_ty);
        let concrete_ty = self.ctx.types.resolve(concrete_ty);
        match (self.ctx.types.get(pattern_ty), self.ctx.types.get(concrete_ty)) {
            (&SemanticType::GenericParam(gp), _) => {
                subst.insert(gp, concrete_ty);
            }
            (&SemanticType::Struct(s1, ref args1, _), &SemanticType::Struct(s2, ref args2, _)) if s1 == s2 => {
                for (&a1, &a2) in args1.iter().zip(args2.iter()) {
                    self.match_types(a1, a2, subst);
                }
            }
            (&SemanticType::Enum(e1, ref args1, _), &SemanticType::Enum(e2, ref args2, _)) if e1 == e2 => {
                for (&a1, &a2) in args1.iter().zip(args2.iter()) {
                    self.match_types(a1, a2, subst);
                }
            }
            (&SemanticType::Tuple(ref args1), &SemanticType::Tuple(ref args2)) => {
                for (&a1, &a2) in args1.iter().zip(args2.iter()) {
                    self.match_types(a1, a2, subst);
                }
            }
            (&SemanticType::Reference(_, _, inner1), &SemanticType::Reference(_, _, inner2)) |
            (&SemanticType::Pointer(_, inner1), &SemanticType::Pointer(_, inner2)) => {
                self.match_types(inner1, inner2, subst);
            }
            _ => {}
        }
    }

    fn normalize_type(&mut self, ty_id: SemanticTypeId) -> SemanticTypeId {
        let ty_id = self.ctx.types.resolve(ty_id);
        let ty = self.ctx.types.get(ty_id).clone();
        match ty {
            SemanticType::Projection { self_type, trait_id, assoc_type } => {
                let norm_self = self.normalize_type(self_type);
                self.normalize_projection(norm_self, trait_id, assoc_type)
            }
            SemanticType::Struct(sym, args, fields) => {
                let mut changed = false;
                let mut new_args = Vec::with_capacity(args.len());
                for a in args {
                    let na = self.normalize_type(a);
                    if na != a { changed = true; }
                    new_args.push(na);
                }
                let mut new_fields = Vec::with_capacity(fields.len());
                for f in fields {
                    let nf = self.normalize_type(f);
                    if nf != f { changed = true; }
                    new_fields.push(nf);
                }
                if changed {
                    self.ctx.types.intern(SemanticType::Struct(sym, new_args, new_fields))
                } else {
                    ty_id
                }
            }
            SemanticType::Enum(sym, args, variants) => {
                let mut changed = false;
                let mut new_args = Vec::with_capacity(args.len());
                for a in args {
                    let na = self.normalize_type(a);
                    if na != a { changed = true; }
                    new_args.push(na);
                }
                let mut new_variants = Vec::with_capacity(variants.len());
                for v in variants {
                    let nv = self.normalize_type(v);
                    if nv != v { changed = true; }
                    new_variants.push(nv);
                }
                if changed {
                    self.ctx.types.intern(SemanticType::Enum(sym, new_args, new_variants))
                } else {
                    ty_id
                }
            }
            SemanticType::Tuple(elems) => {
                let mut changed = false;
                let mut new_elems = Vec::with_capacity(elems.len());
                for e in elems {
                    let ne = self.normalize_type(e);
                    if ne != e { changed = true; }
                    new_elems.push(ne);
                }
                if changed {
                    self.ctx.types.intern(SemanticType::Tuple(new_elems))
                } else {
                    ty_id
                }
            }
            SemanticType::Array(elem, len) => {
                let ne = self.normalize_type(elem);
                if ne != elem {
                    self.ctx.types.intern(SemanticType::Array(ne, len))
                } else {
                    ty_id
                }
            }
            SemanticType::Slice(elem) => {
                let ne = self.normalize_type(elem);
                if ne != elem {
                    self.ctx.types.intern(SemanticType::Slice(ne))
                } else {
                    ty_id
                }
            }
            SemanticType::Pointer(is_rw, inner) => {
                let ni = self.normalize_type(inner);
                if ni != inner {
                    self.ctx.types.intern(SemanticType::Pointer(is_rw, ni))
                } else {
                    ty_id
                }
            }
            SemanticType::Reference(is_rw, lt, inner) => {
                let ni = self.normalize_type(inner);
                if ni != inner {
                    self.ctx.types.intern(SemanticType::Reference(is_rw, lt, ni))
                } else {
                    ty_id
                }
            }
            SemanticType::Future(inner) => {
                let ni = self.normalize_type(inner);
                if ni != inner {
                    self.ctx.types.intern(SemanticType::Future(ni))
                } else {
                    ty_id
                }
            }
            SemanticType::Box(inner) => {
                let ni = self.normalize_type(inner);
                if ni != inner {
                    self.ctx.types.intern(SemanticType::Box(ni))
                } else {
                    ty_id
                }
            }
            SemanticType::Function { params, return_type } => {
                let mut changed = false;
                let mut new_params = Vec::with_capacity(params.len());
                for p in params {
                    let np = self.normalize_type(p);
                    if np != p { changed = true; }
                    new_params.push(np);
                }
                let nr = self.normalize_type(return_type);
                if nr != return_type { changed = true; }
                if changed {
                    self.ctx.types.intern(SemanticType::Function {
                        params: new_params,
                        return_type: nr,
                    })
                } else {
                    ty_id
                }
            }
            _ => ty_id,
        }
    }

    fn substitute(&mut self, ty: SemanticTypeId) -> SemanticTypeId {
        let sub = self.ctx.types.subst(ty, &self.current_subst);
        self.normalize_type(sub)
    }

    fn visit_stmt(&mut self, stmt_id: &StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        match stmt {
            Stmt::Block { body, tail_expr } => {
                for item in body {
                    self.visit_item(item);
                }
                if let Some(expr) = tail_expr {
                    self.visit_expr(expr);
                }
            }
            Stmt::Expr { expr, .. } => {
                self.visit_expr(expr);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.visit_expr(condition);
                self.visit_stmt(then_branch);
                if let Some(else_br) = else_branch {
                    self.visit_stmt(else_br);
                }
            }
            Stmt::While { condition, body, .. } => {
                self.visit_expr(condition);
                self.visit_stmt(body);
            }
            Stmt::For { init, cond, step, body, iterable, pattern, .. } => {
                if let Some(i) = init { self.visit_item(i); }
                if let Some(c) = cond { self.visit_expr(c); }
                if let Some(s) = step { self.visit_expr(s); }
                if let Some(it) = iterable { self.visit_expr(it); }
                if let Some(pat) = pattern { self.visit_pattern(pat); }

                // Record for_loop next instantiation
                if let Some(&next_sym) = self.ctx.tables.for_loop_next.get(stmt_id) {
                    if let Some(&next_decl) = self.ctx.tables.symbol_decls.get(&next_sym) {
                        if let Some(subst) = self.ctx.tables.for_loop_subst.get(stmt_id).cloned() {
                            let mut instance_subst = Vec::new();
                            for (sym, ty) in subst.map {
                                let sub_ty = self.substitute(ty);
                                instance_subst.push((sym, sub_ty));
                            }
                            instance_subst.sort_by_key(|k| k.0);
                            let instance = MonoInstance {
                                decl_id: next_decl,
                                subst: instance_subst,
                                closure_id: None,
                            };
                            self.current_mono_for_loops.insert(*stmt_id, instance.clone());
                            if !self.instantiated.contains_key(&instance) {
                                self.worklist.push(instance);
                            }
                        }
                    }
                }

                self.visit_stmt(body);
            }
            Stmt::Return { value } => {
                if let Some(val) = value {
                    self.visit_expr(val);
                }
            }
            Stmt::Unsafe { body } => {
                self.visit_stmt(body);
            }
            Stmt::Comptime { body } => {
                self.visit_stmt(body);
            }
            // Leaf variants with no children to traverse
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
    }

    fn visit_item(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { body: Some(body_stmt), .. } => {
                        self.visit_stmt(body_stmt);
                    }
                    Decl::Var { initializer, pattern, .. } => {
                        if let Some(expr) = initializer {
                            self.visit_expr(expr);
                        }
                        if let Some(pat_id) = pattern {
                            self.visit_pattern(pat_id);
                        }
                    }
                    _ => {}
                }
            }
            Item::Stmt(stmt_id) => {
                self.visit_stmt(stmt_id);
            }
        }
    }

    fn visit_pattern(&mut self, pat_id: &mellis_ast::PatId) {
        if let Some(&ty) = self.ctx.tables.pat_types.get(pat_id) {
            let sub_ty = self.substitute(ty);
            self.current_pat_types.insert(*pat_id, sub_ty);
        }

        let pattern = &self.arena.pats[pat_id.0 as usize];
        match pattern {
            mellis_ast::Pattern::Tuple { elements, .. } => {
                for elem in elements {
                    self.visit_pattern(elem);
                }
            }
            mellis_ast::Pattern::Struct { fields, .. } => {
                for field in fields {
                    if let Some(field_pat) = field.pattern {
                        self.visit_pattern(&field_pat);
                    }
                }
            }
            mellis_ast::Pattern::Enum { fields, .. } => {
                for field in fields {
                    self.visit_pattern(field);
                }
            }
            // Leaf variants with no children to traverse
            mellis_ast::Pattern::Wildcard | mellis_ast::Pattern::Literal(_) => {}
            mellis_ast::Pattern::Identifier { .. } => {
                if let Some(&sym_id) = self.ctx.tables.pat_symbols.get(pat_id) {
                    if let Some(&ty) = self.ctx.tables.symbol_types.get(&sym_id) {
                        let sub_ty = self.substitute(ty);
                        self.current_symbol_types.insert(sym_id, sub_ty);
                    }
                }
            }
        }
    }

    fn emit_monomorphization_barrier_error(&mut self, ty: SemanticTypeId, expr_id_opt: Option<&ExprId>) {
        let flags = self.ctx.types.type_flags(ty);
        let span = expr_id_opt.and_then(|id| self.get_expr_span_for_diag(id))
            .unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));

        if flags.3 {
            self.ctx.diagnostics.push(
                mellis_common::Diagnostic::error(
                    "E_UNRESOLVED_PROJECTION: Associated type projection could not be normalized at monomorphization barrier"
                ).with_span(span)
            );
        } else if flags.0 {
            self.ctx.diagnostics.push(
                mellis_common::Diagnostic::error(
                    "E_UNCONSTRAINED_INFERENCE: Unconstrained type inference variable reached monomorphization barrier"
                ).with_span(span)
            );
        } else if flags.1 {
            self.ctx.diagnostics.push(
                mellis_common::Diagnostic::error(
                    "E_CANNOT_MONOMORPHIZE: Generic parameter could not be instantiated at monomorphization barrier"
                ).with_span(span)
            );
        } else if flags.2 && self.ctx.diagnostics.is_empty() {
            self.ctx.diagnostics.push(
                mellis_common::Diagnostic::error(
                    "E_CANNOT_MONOMORPHIZE: Semantic error type reached monomorphization barrier"
                ).with_span(span)
            );
        }
    }

    fn get_expr_span_for_diag(&self, expr_id: &ExprId) -> Option<mellis_common::Span> {
        let expr = self.arena.exprs.get(expr_id.0 as usize)?;
        match expr {
            Expr::Literal(tok, _) => Some(tok.span),
            Expr::Identifier { segments, .. } => segments.first().copied(),
            Expr::Call { callee, .. } => self.get_expr_span_for_diag(callee),
            Expr::MethodCall { method_name, .. } => Some(*method_name),
            Expr::Await { expr } => self.get_expr_span_for_diag(expr),
            Expr::Try { expr, .. } => self.get_expr_span_for_diag(expr),
            Expr::Binary { left, right, .. } => self.get_expr_span_for_diag(left).or_else(|| self.get_expr_span_for_diag(right)),
            Expr::Assign { lvalue, value, .. } => self.get_expr_span_for_diag(lvalue).or_else(|| self.get_expr_span_for_diag(value)),
            _ => None,
        }
    }

    fn visit_expr(&mut self, expr_id: &ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        
        // Save the substituted type of the expression
        if let Some(&ty) = self.ctx.tables.expr_types.get(expr_id) {
            let sub_ty = self.substitute(ty);
            self.current_expr_types.insert(*expr_id, sub_ty);
        }

        match expr {
            Expr::Call { callee, args, .. } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(&arg.value);
                }

                // Generic Call Instantiation
                if let Some(&sym_id) = self.ctx.tables.expr_symbols.get(callee) {
                    let symbol = self.ctx.symbol_table.get_symbol(sym_id);
                    if matches!(symbol.kind, crate::SymbolKind::Function | crate::SymbolKind::ExternFunction) {
                        if let Some(&decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                            if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id).cloned() {
                                let mut instance_subst = Vec::new();
                                for (sym, ty) in subst.map {
                                    let sub_ty = self.substitute(ty);
                                    instance_subst.push((sym, sub_ty));
                                }
                                instance_subst.sort_by_key(|k| k.0);
                                let instance = MonoInstance {
                                    decl_id,
                                    subst: instance_subst,
                                    closure_id: None,
                                };
                                self.current_mono_calls.insert(*expr_id, instance.clone());
                                if !self.instantiated.contains_key(&instance) {
                                    self.worklist.push(instance);
                                }
                            } else {
                                // Non-generic call, record it anyway with empty subst
                                let instance = MonoInstance {
                                    decl_id,
                                    subst: vec![],
                                    closure_id: None,
                                };
                                self.current_mono_calls.insert(*expr_id, instance.clone());
                                if !self.instantiated.contains_key(&instance) {
                                    self.worklist.push(instance);
                                }
                            }
                        }
                    }
                }
            }
            Expr::MethodCall { object, args, .. } => {
                self.visit_expr(object);
                for arg in args {
                    self.visit_expr(&arg.value);
                }
                // Handle instantiation for generic method calls
                if let Some(&sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {
                    if let Some(&decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                        let mut instance_subst = Vec::new();
                        if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id).cloned() {
                            for (sym, ty) in subst.map {
                                let sub_ty = self.substitute(ty);
                                instance_subst.push((sym, sub_ty));
                            }
                        }
                        instance_subst.sort_by_key(|k| k.0);
                        let instance = MonoInstance {
                            decl_id,
                            subst: instance_subst,
                            closure_id: None,
                        };
                        self.current_mono_calls.insert(*expr_id, instance.clone());
                        if !self.instantiated.contains_key(&instance) {
                            self.worklist.push(instance);
                        }
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Assign { lvalue, value, .. } => {
                self.visit_expr(lvalue);
                self.visit_expr(value);
            }
            Expr::Member { object, .. } => {
                self.visit_expr(object);
            }
            Expr::Lambda { body, .. } => {
                self.visit_stmt(body);
                
                let mut instance_subst = Vec::new();
                for (&sym, &ty) in &self.current_subst.map {
                    instance_subst.push((sym, ty));
                }
                instance_subst.sort_by_key(|k| k.0);
                
                let decl_id = self.current_instance.as_ref().map(|i| i.decl_id).unwrap_or(mellis_ast::DeclId(0));
                let instance = MonoInstance {
                    decl_id,
                    subst: instance_subst,
                    closure_id: Some(*expr_id),
                };
                if !self.instantiated.contains_key(&instance) {
                    self.worklist.push(instance);
                }
            }
            Expr::StructInit { fields, .. } => {
                for field in fields {
                    self.visit_expr(&field.value);
                }
            }
            Expr::TupleLiteral { elements } => {
                for element in elements {
                    self.visit_expr(element);
                }
            }
            Expr::ArrayLiteral { elements } => {
                for element in elements {
                    self.visit_expr(element);
                }
            }
            Expr::Cast { expr: inner, .. } => {
                self.visit_expr(inner);
            }
            Expr::Unary { operand, .. } => {
                self.visit_expr(operand);
            }
            Expr::Index { base, index } => {
                self.visit_expr(base);
                self.visit_expr(index);
            }
            Expr::TupleIndex { object, .. } => {
                self.visit_expr(object);
            }
            Expr::Match { subject, arms, .. } => {
                self.visit_expr(subject);
                for arm in arms {
                    self.visit_pattern(&arm.pattern);
                    self.visit_stmt(&arm.body);
                }
            }
            Expr::Try { expr: inner, .. } | Expr::Await { expr: inner } => {
                self.visit_expr(inner);
            }
            Expr::Comptime { body } => {
                self.visit_stmt(body);
            }
            // Leaf variants with no children to traverse
            Expr::Literal(_, _) | Expr::Identifier { .. } | Expr::Sizeof { .. } | Expr::Alignof { .. } | Expr::MacroCall { .. } => {}
        }
    }
}
