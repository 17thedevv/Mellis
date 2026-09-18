use luna_ast::{AstArena, Item, Stmt, Expr, Decl, DeclId, ExprId, StmtId};
use crate::{SemanticContext, ty::{SemanticTypeId, SemanticType, Substitution}};
use luna_common::ids::SymbolId;
use std::collections::{HashSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoInstance {
    pub decl_id: DeclId,
    pub subst: Vec<(SymbolId, SemanticTypeId)>,
    pub closure_id: Option<luna_ast::ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CanonicalInstanceKind {
    Decl(DeclId),
    DropGlue {
        struct_sym: SymbolId,
        concrete_ty: SemanticTypeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalInstanceIdentity {
    pub kind: CanonicalInstanceKind,
    pub subst: Vec<(SymbolId, SemanticTypeId)>,
}

pub fn canonical_type_mangling(types: &crate::ty::TypeContext, symbol_table: &crate::SymbolTable, ty_id: SemanticTypeId) -> String {
    crate::Mangler::mangle_type(types, symbol_table, ty_id)
}

impl CanonicalInstanceIdentity {
    pub fn symbol_name_with_tables(
        &self,
        types: &crate::ty::TypeContext,
        symbol_table: &crate::SymbolTable,
        tables: &crate::SemanticTables,
        nominal_name: &str,
    ) -> String {
        match self.kind {
            CanonicalInstanceKind::Decl(decl_id) => {
                if nominal_name == "main" || nominal_name.starts_with("__mellis_") {
                    return nominal_name.to_string();
                }
                let mut found_sym = None;
                for s in &symbol_table.symbols {
                    if s.decl_id == Some(decl_id) {
                        found_sym = Some(s);
                        break;
                    }
                }
                let subst_types: Vec<_> = self.subst.iter().map(|&(_, ty)| ty).collect();
                if let Some(s) = found_sym {
                    if let Some(impl_key) = tables.method_impls.get(&s.id) {
                        let self_path = match impl_key.self_type_def {
                            crate::semantic_tables::ImplSelfTypeKey::Nominal(sym) => symbol_table.get_full_logical_path(sym),
                            crate::semantic_tables::ImplSelfTypeKey::Primitive(b) => vec![format!("{:?}", b).to_lowercase()],
                        };
                        if let Some(trait_sym) = impl_key.trait_id {
                            let trait_path = symbol_table.get_full_logical_path(trait_sym);
                            let self_ty = tables.impl_self_types.get(impl_key).copied().unwrap_or(SemanticTypeId(0));
                            return crate::Mangler::mangle_trait_method(
                                types,
                                symbol_table,
                                &trait_path,
                                &[],
                                self_ty,
                                &s.name,
                                &subst_types,
                            );
                        } else {
                            return crate::Mangler::mangle_method(
                                types,
                                symbol_table,
                                &self_path,
                                &s.name,
                                &subst_types,
                            );
                        }
                    }
                    let full_path = symbol_table.get_full_logical_path(s.id);
                    return crate::Mangler::mangle_function(types, symbol_table, &full_path, &subst_types);
                }
                let full_path = vec![nominal_name.to_string()];
                crate::Mangler::mangle_function(types, symbol_table, &full_path, &subst_types)
            }
            CanonicalInstanceKind::DropGlue { concrete_ty, .. } => {
                crate::Mangler::mangle_drop_glue(types, symbol_table, concrete_ty)
            }
        }
    }

    pub fn symbol_name(&self, types: &crate::ty::TypeContext, symbol_table: &crate::SymbolTable, nominal_name: &str) -> String {
        match self.kind {
            CanonicalInstanceKind::Decl(decl_id) => {
                if nominal_name == "main" || nominal_name.starts_with("__mellis_") {
                    return nominal_name.to_string();
                }
                let mut full_path = Vec::new();
                for s in &symbol_table.symbols {
                    if s.decl_id == Some(decl_id) {
                        full_path = symbol_table.get_full_logical_path(s.id);
                        break;
                    }
                }
                if full_path.is_empty() {
                    full_path = vec![nominal_name.to_string()];
                }
                let subst_types: Vec<_> = self.subst.iter().map(|&(_, ty)| ty).collect();
                crate::Mangler::mangle_function(types, symbol_table, &full_path, &subst_types)
            }
            CanonicalInstanceKind::DropGlue { concrete_ty, .. } => {
                crate::Mangler::mangle_drop_glue(types, symbol_table, concrete_ty)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstantiatedFunction {
    pub instance: MonoInstance,
    pub expr_types: HashMap<ExprId, SemanticTypeId>,
    pub symbol_types: HashMap<SymbolId, SemanticTypeId>,
    pub pat_types: HashMap<luna_ast::PatId, SemanticTypeId>,
    pub mono_calls: HashMap<ExprId, MonoInstance>,
    pub mono_for_loops: HashMap<StmtId, MonoInstance>,
    pub closure_capture_bindings: Vec<crate::semantic_tables::CaptureBinding>,
    pub closure_env_type: Option<SemanticTypeId>,
    pub closure_env_ptr_type: Option<SemanticTypeId>,
}

pub struct MonoCollector<'a> {
    ctx: &'a mut SemanticContext,
    arena: &'a AstArena,
    source_manager: Option<&'a luna_common::source::SourceManager>,
    
    pub worklist: Vec<MonoInstance>,
    pub instantiated: HashMap<MonoInstance, InstantiatedFunction>,
    pub drop_glues: Vec<CanonicalInstanceIdentity>,
    pub visited_drop_types: HashSet<SemanticTypeId>,
    
    // Temporary state
    current_instance: Option<MonoInstance>,
    current_expr_types: HashMap<ExprId, SemanticTypeId>,
    current_symbol_types: HashMap<SymbolId, SemanticTypeId>,
    current_pat_types: HashMap<luna_ast::PatId, SemanticTypeId>,
    current_mono_calls: HashMap<ExprId, MonoInstance>,
    current_mono_for_loops: HashMap<StmtId, MonoInstance>,
    current_subst: Substitution,
}

impl<'a> MonoCollector<'a> {
    pub fn new(ctx: &'a mut SemanticContext, arena: &'a AstArena) -> Self {
        Self::new_with_source(ctx, arena, None)
    }

    pub fn new_with_source(ctx: &'a mut SemanticContext, arena: &'a AstArena, source_manager: Option<&'a luna_common::source::SourceManager>) -> Self {
        Self {
            ctx,
            arena,
            source_manager,
            worklist: Vec::new(),
            instantiated: HashMap::new(),
            drop_glues: Vec::new(),
            visited_drop_types: HashSet::new(),
            current_instance: None,
            current_expr_types: HashMap::new(),
            current_symbol_types: HashMap::new(),
            current_pat_types: HashMap::new(),
            current_mono_calls: HashMap::new(),
            current_mono_for_loops: HashMap::new(),
            current_subst: Substitution::new(),
        }
    }

    pub fn get_span_text(&self, span: luna_common::ids::Span) -> &'a str {
        if let Some(sm) = self.source_manager {
            if let Some(f) = sm.get_file(span.file_id) {
                let start = span.start as usize;
                let end = span.end as usize;
                if start <= end && end <= f.source.len() {
                    return &f.source[start..end];
                }
            }
        }
        ""
    }

    pub fn discover_drop_obligations(&mut self, ty_id: SemanticTypeId) {
        let concrete_ty = self.ctx.types.resolve(ty_id);
        if !self.ctx.types.is_monomorphic(concrete_ty) {
            return;
        }
        if !self.ctx.needs_drop(concrete_ty) {
            return;
        }
        if !self.visited_drop_types.insert(concrete_ty) {
            return;
        }

        match self.ctx.types.get(concrete_ty).clone() {
            SemanticType::Struct(sym_id, _, fields) => {
                let drop_sym_opt = self.ctx.lang_items.get(crate::lang_item::LangItem::Drop);
                let mut instance_subst = Vec::new();
                if let Some(drop_sym) = drop_sym_opt {
                    let impl_key = crate::semantic_tables::ImplKey {
                        trait_id: Some(drop_sym),
                        self_type_def: sym_id.into(),
                    };
                    if let Some(impl_decls) = self.ctx.tables.trait_impls.get(&impl_key).cloned() {
                        for impl_decl_id in impl_decls {
                            if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                let mut subst = crate::ty::Substitution::new();
                                if let Some(&pattern_ty) = self.ctx.tables.impl_self_types.get(&impl_key) {
                                    self.match_types(pattern_ty, concrete_ty, &mut subst);
                                }
                                instance_subst = subst.map.into_iter().collect();
                                instance_subst.sort_by_key(|k| k.0);
                                for &m_decl_id in methods {
                                    let instance = MonoInstance {
                                        decl_id: m_decl_id,
                                        subst: instance_subst.clone(),
                                        closure_id: None,
                                    };
                                    if !self.instantiated.contains_key(&instance) && !self.worklist.contains(&instance) {
                                        self.worklist.push(instance);
                                    }
                                }
                            }
                        }
                    }
                }

                let glue_id = CanonicalInstanceIdentity {
                    kind: CanonicalInstanceKind::DropGlue {
                        struct_sym: sym_id,
                        concrete_ty,
                    },
                    subst: instance_subst,
                };
                if !self.drop_glues.contains(&glue_id) {
                    self.drop_glues.push(glue_id);
                }

                self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Mutable, concrete_ty));

                for field_ty in fields {
                    self.discover_drop_obligations(field_ty);
                }
            }
            SemanticType::Enum(sym_id, _, variants) => {
                let drop_sym_opt = self.ctx.lang_items.get(crate::lang_item::LangItem::Drop);
                let mut instance_subst = Vec::new();
                if let Some(drop_sym) = drop_sym_opt {
                    let impl_key = crate::semantic_tables::ImplKey {
                        trait_id: Some(drop_sym),
                        self_type_def: sym_id.into(),
                    };
                    if let Some(impl_decls) = self.ctx.tables.trait_impls.get(&impl_key).cloned() {
                        for impl_decl_id in impl_decls {
                            if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                let mut subst = crate::ty::Substitution::new();
                                if let Some(&pattern_ty) = self.ctx.tables.impl_self_types.get(&impl_key) {
                                    self.match_types(pattern_ty, concrete_ty, &mut subst);
                                }
                                instance_subst = subst.map.into_iter().collect();
                                instance_subst.sort_by_key(|k| k.0);
                                for &m_decl_id in methods {
                                    let instance = MonoInstance {
                                        decl_id: m_decl_id,
                                        subst: instance_subst.clone(),
                                        closure_id: None,
                                    };
                                    if !self.instantiated.contains_key(&instance) && !self.worklist.contains(&instance) {
                                        self.worklist.push(instance);
                                    }
                                }
                            }
                        }
                    }
                }

                let glue_id = CanonicalInstanceIdentity {
                    kind: CanonicalInstanceKind::DropGlue {
                        struct_sym: sym_id,
                        concrete_ty,
                    },
                    subst: instance_subst,
                };
                if !self.drop_glues.contains(&glue_id) {
                    self.drop_glues.push(glue_id);
                }

                self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Mutable, concrete_ty));

                for var_ty in variants {
                    self.discover_drop_obligations(var_ty);
                }
            }
            SemanticType::Tuple(elems) => {
                if self.ctx.needs_drop(concrete_ty) {
                    let glue_id = CanonicalInstanceIdentity {
                        kind: CanonicalInstanceKind::DropGlue {
                            struct_sym: luna_common::ids::SymbolId(0),
                            concrete_ty,
                        },
                        subst: Vec::new(),
                    };
                    if !self.drop_glues.contains(&glue_id) {
                        self.drop_glues.push(glue_id);
                    }
                    self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Mutable, concrete_ty));
                }
                for elem_ty in elems {
                    self.discover_drop_obligations(elem_ty);
                }
            }
            SemanticType::Array(elem_ty, _) => {
                self.discover_drop_obligations(elem_ty);
            }
            _ => {}
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
                    Decl::Impl { generic_params, methods, .. } => {
                        if generic_params.is_empty() {
                            for m_id in methods {
                                if let Decl::Function { generic_params: m_gps, .. } = &self.arena.decls[m_id.0 as usize] {
                                    if m_gps.is_empty() {
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
                    self_type_def: (*concrete_sym).into(),
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
                        self_type_def: (*concrete_sym).into(),
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

            if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(&instance.decl_id) {
                eprintln!("MONO INSTANCE DECL: decl_id={:?} sym_name={}", instance.decl_id, self.ctx.symbol_table.get_symbol(sym_id).name);
            } else {
                eprintln!("MONO INSTANCE DECL: decl_id={:?} (NO SYMBOL)", instance.decl_id);
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

            // Remap substitution to the function's own generic param symbols.
            // The instance.subst uses call-site generic param symbols, but the
            // function body may use its own generic param symbols (different SymbolIds).
            // We look up the function's generic_param_symbols and map them too.
            if !instance.subst.is_empty() {
                let decl_id = instance.decl_id;
                let impl_decl_id = self.ctx.tables.decl_symbols.get(&decl_id).and_then(|&sym_id| {
                    self.ctx.tables.method_impls.get(&sym_id).and_then(|key| {
                        self.ctx.tables.trait_impls.get(key).and_then(|decls| decls.first().copied())
                    })
                });

                // Collect ALL generic param SymbolIds from impl + function declarations
                let mut scope_gp_syms = Vec::new();
                if let Some(impl_id) = impl_decl_id {
                    let mut impl_gp_idx = 0;
                    while let Some(&impl_gp_sym) = self.ctx.tables.generic_param_symbols.get(&(impl_id, impl_gp_idx)) {
                        scope_gp_syms.push(impl_gp_sym);
                        impl_gp_idx += 1;
                    }
                }
                {
                    let mut fn_gp_idx = 0;
                    while let Some(&fn_gp_sym) = self.ctx.tables.generic_param_symbols.get(&(decl_id, fn_gp_idx)) {
                        scope_gp_syms.push(fn_gp_sym);
                        fn_gp_idx += 1;
                    }
                }

                // Positional remapping: map each scoped generic param from the ordered subst
                for (idx, &gp_sym) in scope_gp_syms.iter().enumerate() {
                    if idx < instance.subst.len() {
                        let (_, concrete_ty) = instance.subst[idx];
                        self.current_subst.insert(gp_sym, concrete_ty);
                    }
                }

                // Name-based remapping: for each subst entry, find any scoped generic param
                // with a matching name that hasn't been mapped yet, and map it too.
                // This handles cases where the function body references generic params via
                // different SymbolIds (e.g., from trait declarations) that share the same name.
                for &(sym, ty) in &instance.subst {
                    let sym_name = if (sym.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        self.ctx.symbol_table.symbols[sym.0 as usize].name.clone()
                    } else { continue };
                    if sym_name.is_empty() { continue; }
                    for &gp_sym in &scope_gp_syms {
                        if !self.current_subst.map.contains_key(&gp_sym) {
                            let gp_name = if (gp_sym.0 as usize) < self.ctx.symbol_table.symbols.len() {
                                &self.ctx.symbol_table.symbols[gp_sym.0 as usize].name
                            } else { continue };
                            if *gp_name == sym_name {
                                self.current_subst.insert(gp_sym, ty);
                            }
                        }
                    }
                }
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
                                self.discover_drop_obligations(sub_ty);
                            }
                        }
                    }
                    self.visit_stmt(body);
                }
            } else {
                if (instance.decl_id.0 as usize) < self.arena.decls.len() {
                    let decl = &self.arena.decls[instance.decl_id.0 as usize];
                    match decl {
                        Decl::Function { body: Some(body_stmt), params, is_intrinsic, name: fn_name, .. } => {
                            eprintln!("MONO FN BODY: decl_id={:?} is_intrinsic={} name_span={:?}", instance.decl_id, is_intrinsic, fn_name);
                            if let Some(&fn_sym) = self.ctx.tables.decl_symbols.get(&instance.decl_id) {
                                if let Some(&fn_ty) = self.ctx.tables.symbol_types.get(&fn_sym) {
                                    let sub_fn_ty = self.substitute(fn_ty);
                                    self.current_symbol_types.insert(fn_sym, sub_fn_ty);
                                }
                            }
                            for param_id in params {
                                if let Some(&param_sym) = self.ctx.tables.decl_symbols.get(param_id) {
                                    if let Some(&ty) = self.ctx.tables.symbol_types.get(&param_sym) {
                                        let sub_ty = self.substitute(ty);
                                        self.current_symbol_types.insert(param_sym, sub_ty);
                                        self.discover_drop_obligations(sub_ty);
                                    }
                                }
                            }
                            self.visit_stmt(body_stmt);
                        },
                        Decl::Function { body: None, is_intrinsic, name: fn_name, .. } => {
                            eprintln!("MONO FUNCTION NO BODY: decl_id={:?} is_intrinsic={} name_span={:?}", instance.decl_id, is_intrinsic, fn_name);
                        },
                        _ => {}
                    }
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
                if let luna_ast::Expr::Identifier { .. } | luna_ast::Expr::Member { .. } = &self.arena.exprs[expr_id.0 as usize] {
                    if matches!(self.ctx.types.get(resolved), crate::ty::SemanticType::Function { .. } | crate::ty::SemanticType::Enum(_, _, _)) {
                        continue;
                    }
                }

                if !self.ctx.types.is_monomorphic(resolved) {
                    eprintln!("BARRIER ERROR EXPR: expr={:?} ty={:?} ({:?}) current_subst={:?} decl={:?}", expr_id, resolved, self.ctx.types.get(resolved), self.current_subst, instance.decl_id);
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
                    let sym_name = if (sym_id.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        self.ctx.symbol_table.symbols[sym_id.0 as usize].name.clone()
                    } else { String::new() };
                    eprintln!("BARRIER ERROR SYM: sym={:?} name={} ty={:?} ({:?}) current_subst={:?} decl={:?}", sym_id, sym_name, resolved, self.ctx.types.get(resolved), self.current_subst, instance.decl_id);
                    self.emit_monomorphization_barrier_error(resolved, None);
                    is_concrete = false;
                }
            }

            let mut current_pat_types = std::mem::take(&mut self.current_pat_types);
            for (pat_id, ty) in current_pat_types.iter_mut() {
                let resolved = self.substitute(*ty);
                *ty = resolved;
                if !self.ctx.types.is_monomorphic(resolved) {
                    eprintln!("BARRIER ERROR PAT: pat={:?} ty={:?} ({:?}) current_subst={:?} decl={:?}", pat_id, resolved, self.ctx.types.get(resolved), self.current_subst, instance.decl_id);
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
            self_type_def: nominal_sym.into(),
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

    fn order_subst_for_decl(
        &mut self,
        decl_id: DeclId,
        impl_decl_id: Option<DeclId>,
        subst: Vec<(SymbolId, SemanticTypeId)>,
    ) -> Vec<(SymbolId, SemanticTypeId)> {
        let mut ordered = Vec::new();
        let mut used = vec![false; subst.len()];

        let mut expected_symbols = Vec::new();
        if let Some(impl_id) = impl_decl_id {
            let mut idx = 0;
            while let Some(&sym) = self.ctx.tables.generic_param_symbols.get(&(impl_id, idx)) {
                expected_symbols.push(sym);
                idx += 1;
            }
        }
        let mut idx = 0;
        while let Some(&sym) = self.ctx.tables.generic_param_symbols.get(&(decl_id, idx)) {
            expected_symbols.push(sym);
            idx += 1;
        }

        eprintln!("[ORDER_SUBST] decl_id={:?}, impl_id={:?}, expected_symbols={:?}, input_subst={:?}", decl_id, impl_decl_id, expected_symbols, subst);
        for exp_sym in expected_symbols {
            let exp_name = if (exp_sym.0 as usize) < self.ctx.symbol_table.symbols.len() {
                &self.ctx.symbol_table.symbols[exp_sym.0 as usize].name
            } else {
                continue;
            };
            let mut found = None;
            for (i, (s, ty)) in subst.iter().enumerate() {
                if !used[i] {
                    let s_name = if (s.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        &self.ctx.symbol_table.symbols[s.0 as usize].name
                    } else {
                        ""
                    };
                    if *s == exp_sym || s_name == exp_name {
                        found = Some((i, (*s, *ty)));
                        break;
                    }
                }
            }
            if let Some((i, (_s, ty))) = found {
                used[i] = true;
                ordered.push((exp_sym, ty));
            } else if let Some(&sub_ty) = self.current_subst.get(exp_sym) {
                ordered.push((exp_sym, sub_ty));
            } else {
                let gen_ty = self.ctx.types.intern(SemanticType::GenericParam(exp_sym));
                let sub_ty = self.ctx.types.subst(gen_ty, &self.current_subst);
                if sub_ty != gen_ty {
                    ordered.push((exp_sym, sub_ty));
                }
            }
        }

        for (i, pair) in subst.into_iter().enumerate() {
            if !used[i] {
                let name = if (pair.0.0 as usize) < self.ctx.symbol_table.symbols.len() {
                    &self.ctx.symbol_table.symbols[pair.0.0 as usize].name
                } else {
                    ""
                };
                if !name.is_empty() && !ordered.iter().any(|(s, _)| {
                    if (s.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        self.ctx.symbol_table.symbols[s.0 as usize].name == *name
                    } else {
                        false
                    }
                }) {
                    ordered.push(pair);
                }
            }
        }

        eprintln!("[ORDER_SUBST] decl_id={:?}, result_ordered={:?}", decl_id, ordered);
        ordered
    }

    fn substitute(&mut self, ty: SemanticTypeId) -> SemanticTypeId {
        let resolved = self.ctx.types.resolve(ty);
        let sem_ty = self.ctx.types.get(resolved).clone();
        
        let sub = match sem_ty {
            crate::ty::SemanticType::GenericParam(gp_sym) => {
                if let Some(&new_id) = self.current_subst.get(gp_sym) {
                    if new_id == resolved { resolved } else { self.substitute(new_id) }
                } else {
                    // Name fallback
                    let gp_name = if (gp_sym.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        self.ctx.symbol_table.symbols[gp_sym.0 as usize].name.clone()
                    } else {
                        String::new()
                    };
                    if gp_name.is_empty() {
                        resolved
                    } else {
                        let mut found = None;
                        for (&sym, &concrete_ty) in &self.current_subst.map {
                            let sym_name = if (sym.0 as usize) < self.ctx.symbol_table.symbols.len() {
                                &self.ctx.symbol_table.symbols[sym.0 as usize].name
                            } else {
                                continue;
                            };
                            if *sym_name == gp_name {
                                found = Some(concrete_ty);
                                break;
                            }
                        }
                        if let Some(concrete_ty) = found {
                            self.current_subst.insert(gp_sym, concrete_ty);
                            self.substitute(concrete_ty)
                        } else {
                            resolved
                        }
                    }
                }
            }
            crate::ty::SemanticType::Struct(sym, args, fields) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.substitute(a)).collect();
                let new_fields: Vec<_> = fields.iter().map(|&f| self.substitute(f)).collect();
                self.ctx.types.intern(crate::ty::SemanticType::Struct(sym, new_args, new_fields))
            }
            crate::ty::SemanticType::Enum(sym, args, variants) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.substitute(a)).collect();
                let new_variants: Vec<_> = variants.iter().map(|&v| self.substitute(v)).collect();
                self.ctx.types.intern(crate::ty::SemanticType::Enum(sym, new_args, new_variants))
            }
            crate::ty::SemanticType::Tuple(args) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.substitute(a)).collect();
                self.ctx.types.intern(crate::ty::SemanticType::Tuple(new_args))
            }
            crate::ty::SemanticType::Function { params, return_type } => {
                let new_params: Vec<_> = params.iter().map(|&p| self.substitute(p)).collect();
                let new_ret = self.substitute(return_type);
                self.ctx.types.intern(crate::ty::SemanticType::Function { params: new_params, return_type: new_ret })
            }
            crate::ty::SemanticType::Pointer(mutability, inner) => {
                let new_inner = self.substitute(inner);
                self.ctx.types.intern(crate::ty::SemanticType::Pointer(mutability, new_inner))
            }
            crate::ty::SemanticType::Reference(lt, mutability, inner) => {
                let new_inner = self.substitute(inner);
                self.ctx.types.intern(crate::ty::SemanticType::Reference(lt, mutability, new_inner))
            }
            crate::ty::SemanticType::Closure(expr_id, params, return_type) => {
                let new_params = params.iter().map(|&p| self.substitute(p)).collect();
                let new_ret = self.substitute(return_type);
                self.ctx.types.intern(crate::ty::SemanticType::Closure(expr_id, new_params, new_ret))
            }
            crate::ty::SemanticType::Future(inner) => {
                let new_inner = self.substitute(inner);
                self.ctx.types.intern(crate::ty::SemanticType::Future(new_inner))
            }
            crate::ty::SemanticType::Projection { self_type, trait_id, assoc_type } => {
                let new_self = self.substitute(self_type);
                self.ctx.types.intern(crate::ty::SemanticType::Projection { self_type: new_self, trait_id, assoc_type })
            }
            crate::ty::SemanticType::Slice(inner) => {
                let new_inner = self.substitute(inner);
                self.ctx.types.intern(crate::ty::SemanticType::Slice(new_inner))
            }
            crate::ty::SemanticType::Array(inner, len) => {
                let new_inner = self.substitute(inner);
                self.ctx.types.intern(crate::ty::SemanticType::Array(new_inner, len))
            }
            _ => resolved,
        };
        
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
                        if let Some(decl_sym) = self.ctx.tables.decl_symbols.get(decl_id).copied() {
                            if let Some(&ty) = self.ctx.tables.symbol_types.get(&decl_sym) {
                                let sub_ty = self.substitute(ty);
                                self.discover_drop_obligations(sub_ty);
                            }
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

    fn visit_pattern(&mut self, pat_id: &luna_ast::PatId) {
        eprintln!("[VISIT_PAT] in inst={:?}, pat_id={:?}, pattern={:?}", self.current_instance.as_ref().map(|i| i.decl_id), pat_id, &self.arena.pats[pat_id.0 as usize]);
        if let Some(&ty) = self.ctx.tables.pat_types.get(pat_id) {
            let sub_ty = self.substitute(ty);
            self.current_pat_types.insert(*pat_id, sub_ty);
            self.discover_drop_obligations(sub_ty);
        }

        let pattern = &self.arena.pats[pat_id.0 as usize];
        match pattern {
            luna_ast::Pattern::Tuple { elements, .. } => {
                for elem in elements {
                    self.visit_pattern(elem);
                }
            }
            luna_ast::Pattern::Struct { fields, .. } => {
                for field in fields {
                    if let Some(field_pat) = field.pattern {
                        self.visit_pattern(&field_pat);
                    }
                }
            }
            luna_ast::Pattern::Enum { fields, .. } => {
                for field in fields {
                    self.visit_pattern(field);
                }
            }
            // Leaf variants with no children to traverse
            luna_ast::Pattern::Wildcard | luna_ast::Pattern::Literal(_) => {}
            luna_ast::Pattern::Identifier { .. } => {
                let pat_sym = self.ctx.tables.pat_symbols.get(pat_id).copied();
                if let Some(sym_id) = pat_sym {
                    let ty_opt = self.ctx.tables.symbol_types.get(&sym_id).copied()
                        .or_else(|| self.ctx.tables.pat_types.get(pat_id).copied());
                    if let Some(ty) = ty_opt {
                        let sub_ty = self.substitute(ty);
                        if format!("{:?}", self.ctx.types.get(ty)).contains("GenericParam") {
                            let (orig_name, decl_id, span) = if let SemanticType::GenericParam(gp) = self.ctx.types.get(ty) {
                                let s = self.ctx.symbol_table.get_symbol(*gp);
                                (s.name.clone(), s.decl_id, s.span)
                            } else { ("".to_string(), None, luna_common::ids::Span::new(luna_common::ids::FileId(0), 0, 0)) };
                            let pat_s = self.ctx.symbol_table.get_symbol(sym_id);
                            eprintln!("[DEBUG ZIP MONO] in inst={:?}, pat_sym={:?} (name={}, span={:?}), orig_ty={:?} (name={}, decl={:?}, span={:?}), sub_ty={:?}",
                                self.current_instance.as_ref().map(|i| i.decl_id), sym_id, pat_s.name, pat_s.span, self.ctx.types.get(ty), orig_name, decl_id, span, self.ctx.types.get(sub_ty));
                        }
                        self.current_symbol_types.insert(sym_id, sub_ty);
                        self.discover_drop_obligations(sub_ty);
                    }
                }
            }
        }
    }

    fn emit_monomorphization_barrier_error(&mut self, ty: SemanticTypeId, expr_id_opt: Option<&ExprId>) {
        let flags = self.ctx.types.type_flags(ty);
        let span = expr_id_opt.and_then(|id| self.get_expr_span_for_diag(id))
            .unwrap_or(luna_common::Span::new(luna_common::ids::FileId(0), 0, 0));

        eprintln!("BARRIER ERROR FILE: {:?}, start: {}, end: {}", span.file_id, span.start, span.end);
        eprintln!("BARRIER ERROR TY: {:?}", self.ctx.types.get(ty));

        if flags.3 {
            self.ctx.diagnostics.push(
                luna_common::Diagnostic::error(
                    "E_UNRESOLVED_PROJECTION: Associated type projection could not be normalized at monomorphization barrier"
                ).with_span(span)
            );
        } else if flags.0 {
            self.ctx.diagnostics.push(
                luna_common::Diagnostic::error(
                    "E_UNCONSTRAINED_INFERENCE: Unconstrained type inference variable reached monomorphization barrier"
                ).with_span(span)
            );
        } else if flags.1 {
            self.ctx.diagnostics.push(
                luna_common::Diagnostic::error(
                    "E_CANNOT_MONOMORPHIZE: Generic parameter could not be instantiated at monomorphization barrier"
                ).with_span(span)
            );
        } else if flags.2 && self.ctx.diagnostics.is_empty() {
            self.ctx.diagnostics.push(
                luna_common::Diagnostic::error(
                    "E_CANNOT_MONOMORPHIZE: Semantic error type reached monomorphization barrier"
                ).with_span(span)
            );
        }
    }

    fn get_expr_span_for_diag(&self, expr_id: &ExprId) -> Option<luna_common::Span> {
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
                    let sym_name = symbol.name.clone();
                    let sym_kind = symbol.kind;
                    if matches!(sym_kind, crate::SymbolKind::Function | crate::SymbolKind::ExternFunction) {
                        if let Some(&decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                            // Verify decl_id matches the actual function
                            if sym_name.contains("drop_in_place") || sym_name.contains("size_of") || sym_name.contains("align_of") {
                                let actual_decl = &self.arena.decls[decl_id.0 as usize];
                                let has_body = matches!(actual_decl, Decl::Function { body: Some(_), .. });
                                let is_intr = matches!(actual_decl, Decl::Function { is_intrinsic: true, .. });
                                eprintln!("CALLEE RESOLVE: sym={} sym_id={:?} -> decl_id={:?}, has_body={}, is_intrinsic={}", 
                                    sym_name, sym_id, decl_id, has_body, is_intr);
                            }
                            if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id).cloned() {
                                let mut instance_subst = Vec::new();
                                for (sym, ty) in subst.map {
                                    let sub_ty = self.substitute(ty);
                                    instance_subst.push((sym, sub_ty));
                                }
                                let instance_subst = self.order_subst_for_decl(decl_id, None, instance_subst);
                                eprintln!("MONO CALL WITH EXPR_SUBST: callee sym={} decl_id={:?}, instance_subst={:?}",
                                    sym_name, decl_id,
                                    instance_subst.iter().map(|(s,t)| (s, format!("{:?}", self.ctx.types.get(*t)))).collect::<Vec<_>>());
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
                                eprintln!("MONO CALL NO EXPR_SUBST: callee sym={} decl_id={:?}, current_inst={:?}, current_subst={:?}",
                                    sym_name, decl_id,
                                    self.current_instance.as_ref().map(|i| i.decl_id),
                                    self.current_subst.map.iter().map(|(&k,&v)| (k, format!("{:?}", self.ctx.types.get(v)))).collect::<Vec<_>>());
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

                // Special check for ptr::drop_in_place<T>(p)
                let callee_name = if let Some(&sym_id) = self.ctx.tables.expr_symbols.get(callee) {
                    if (sym_id.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        Some(self.ctx.symbol_table.symbols[sym_id.0 as usize].name.clone())
                    } else { None }
                } else { None };
                if let Some(ref name) = callee_name {
                    if name.ends_with("drop_in_place") || name.contains("drop_in_place") {
                        if let Some(first_arg) = args.first() {
                            if let Some(&arg_ty) = self.ctx.tables.expr_types.get(&first_arg.value) {
                                let sub_arg_ty = self.substitute(arg_ty);
                                if let SemanticType::Pointer(_, inner) = self.ctx.types.get(sub_arg_ty) {
                                    let pointee = *inner;
                                    self.discover_drop_obligations(pointee);
                                }
                            }
                        }
                    }
                }
                if let Some(&ty) = self.current_expr_types.get(expr_id) {
                    self.discover_drop_obligations(ty);
                }
            }
            Expr::MethodCall { object, args, method_name, .. } => {
                self.visit_expr(object);
                for arg in args {
                    self.visit_expr(&arg.value);
                }

                let obj_ty = self.current_expr_types.get(object).copied()
                    .or_else(|| self.ctx.tables.expr_types.get(object).copied())
                    .unwrap_or(SemanticTypeId(0));
                let sub_obj_ty = self.substitute(obj_ty);
                let peeled_sub_ty = match self.ctx.types.get(sub_obj_ty) {
                    SemanticType::Reference(_, _, inner) | SemanticType::Pointer(_, inner) => *inner,
                    _ => sub_obj_ty,
                };

                let sym_id_opt = if self.ctx.tables.expr_member_indices.contains_key(expr_id) {
                    None
                } else {
                    self.ctx.tables.expr_symbols.get(expr_id).copied().or_else(|| {
                        let m_name_str = self.get_span_text(*method_name);
                        if m_name_str.is_empty() { return None; }
                        let target_key = match self.ctx.types.get(peeled_sub_ty) {
                            SemanticType::Struct(s, ..) | SemanticType::Enum(s, ..) => Some(crate::semantic_tables::ImplSelfTypeKey::Nominal(*s)),
                            SemanticType::Primitive(b) => Some(crate::semantic_tables::ImplSelfTypeKey::Primitive(*b)),
                            _ => None,
                        };
                        if let Some(self_key) = target_key {
                            return self.ctx.tables
                                .find_method_prefer_inherent(self_key, &m_name_str, &self.ctx.symbol_table)
                                .map(|(method, _)| method);
                        }
                        None
                    })
                };

                // Handle instantiation for method calls (including generic trait calls)
                if let Some(sym_id) = sym_id_opt {
                    let trait_owner_opt = self.ctx.tables.trait_methods.iter()
                        .find(|(_, meths)| meths.contains(&sym_id))
                        .map(|(&t_sym, _)| t_sym)
                        .or_else(|| self.ctx.tables.method_impls.get(&sym_id).and_then(|k| k.trait_id));

                    let (target_decl_id, target_impl_id, instance_subst) = if let Some(trait_sym) = trait_owner_opt {
                        let target_key = match self.ctx.types.get(peeled_sub_ty) {
                            SemanticType::Struct(s, ..) | SemanticType::Enum(s, ..) => Some(crate::semantic_tables::ImplSelfTypeKey::Nominal(*s)),
                            SemanticType::Primitive(b) => Some(crate::semantic_tables::ImplSelfTypeKey::Primitive(*b)),
                            _ => None,
                        };
                        let mut concrete_decl = None;
                        let mut concrete_impl = None;
                        let mut impl_subst = crate::ty::Substitution::new();
                        if let Some(self_key) = target_key {
                            let key = crate::semantic_tables::ImplKey {
                                trait_id: Some(trait_sym),
                                self_type_def: self_key,
                            };
                            if let Some(impl_decls) = self.ctx.tables.trait_impls.get(&key) {
                                for &impl_decl_id in impl_decls {
                                    if (impl_decl_id.0 as usize) < self.arena.decls.len() {
                                        if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                            let method_name = self.ctx.symbol_table.get_symbol(sym_id).name.clone();
                                            for &m_decl_id in methods {
                                                if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_decl_id) {
                                                    if self.ctx.symbol_table.get_symbol(m_sym).name == method_name {
                                                        concrete_decl = Some(m_decl_id);
                                                        concrete_impl = Some(impl_decl_id);
                                                        if let Some(&pattern_ty) = self.ctx.tables.impl_self_types.get(&key) {
                                                            self.match_types(pattern_ty, peeled_sub_ty, &mut impl_subst);
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if concrete_decl.is_some() { break; }
                                }
                            }
                        }
                        let mut subst_pairs: Vec<(SymbolId, SemanticTypeId)> = impl_subst.map.into_iter().map(|(k, v)| (k, self.substitute(v))).collect();
                        if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id).cloned() {
                            for (sym, ty) in subst.map {
                                let sub_ty = self.substitute(ty);
                                let sym_name = if (sym.0 as usize) < self.ctx.symbol_table.symbols.len() {
                                    &self.ctx.symbol_table.symbols[sym.0 as usize].name
                                } else {
                                    ""
                                };
                                if !subst_pairs.iter().any(|(s, _)| {
                                    if (s.0 as usize) < self.ctx.symbol_table.symbols.len() {
                                        &self.ctx.symbol_table.symbols[s.0 as usize].name == sym_name
                                    } else {
                                        *s == sym
                                    }
                                }) {
                                    subst_pairs.push((sym, sub_ty));
                                }
                            }
                        }
                        (concrete_decl.or_else(|| self.ctx.tables.symbol_decls.get(&sym_id).copied()), concrete_impl.or_else(|| self.ctx.tables.method_impls.get(&sym_id).and_then(|k| self.ctx.tables.trait_impls.get(k).and_then(|decls| decls.first().copied()))), subst_pairs)
                    } else {
                        let mut impl_subst = crate::ty::Substitution::new();
                        let inherent_impl = self.ctx.tables.method_impls.get(&sym_id).and_then(|key| {
                            if let Some(&pattern_ty) = self.ctx.tables.impl_self_types.get(key) {
                                self.match_types(pattern_ty, peeled_sub_ty, &mut impl_subst);
                            }
                            self.ctx.tables.trait_impls.get(key).and_then(|decls| decls.first().copied())
                        });
                        let mut subst_pairs: Vec<(SymbolId, SemanticTypeId)> = impl_subst.map.into_iter().map(|(k, v)| (k, self.substitute(v))).collect();
                        if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id).cloned() {
                            for (sym, ty) in subst.map {
                                let sub_ty = self.substitute(ty);
                                let sym_name = if (sym.0 as usize) < self.ctx.symbol_table.symbols.len() {
                                    &self.ctx.symbol_table.symbols[sym.0 as usize].name
                                } else {
                                    ""
                                };
                                if !subst_pairs.iter().any(|(s, _)| {
                                    if (s.0 as usize) < self.ctx.symbol_table.symbols.len() {
                                        &self.ctx.symbol_table.symbols[s.0 as usize].name == sym_name
                                    } else {
                                        *s == sym
                                    }
                                }) {
                                    subst_pairs.push((sym, sub_ty));
                                }
                            }
                        }
                        (self.ctx.tables.symbol_decls.get(&sym_id).copied(), inherent_impl, subst_pairs)
                    };

                    if let Some(decl_id) = target_decl_id {
                        let instance_subst = self.order_subst_for_decl(decl_id, target_impl_id, instance_subst);
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
                if let Some(&ty) = self.current_expr_types.get(expr_id) {
                    self.discover_drop_obligations(ty);
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
                
                let decl_id = self.current_instance.as_ref().map(|i| i.decl_id).unwrap_or(luna_ast::DeclId(0));
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
                if let Some(&ty) = self.current_expr_types.get(expr_id) {
                    self.discover_drop_obligations(ty);
                }
            }
            Expr::TupleLiteral { elements } => {
                for element in elements {
                    self.visit_expr(element);
                }
                if let Some(&ty) = self.current_expr_types.get(expr_id) {
                    self.discover_drop_obligations(ty);
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
            Expr::Identifier { .. } => {
                if let Some(&sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {
                    let symbol = self.ctx.symbol_table.get_symbol(sym_id);
                    if matches!(symbol.kind, crate::SymbolKind::Function) {
                        if let Some(&decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                            if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id).cloned() {
                                let mut instance_subst = Vec::new();
                                for (sym, ty) in subst.map {
                                    let sub_ty = self.substitute(ty);
                                    instance_subst.push((sym, sub_ty));
                                }
                                let instance_subst = self.order_subst_for_decl(decl_id, None, instance_subst);
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
                }
            }
            // Leaf variants with no children to traverse
            Expr::Literal(_, _) | Expr::Sizeof { .. } | Expr::Alignof { .. } | Expr::MacroCall { .. } => {}
        }
    }
}
