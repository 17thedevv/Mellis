use luna_ast::Visibility;
use luna_semantic::symbol::{ProviderId, ScopeKind, Symbol, SymbolKind};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ExternalSymbol {
    pub sym: Symbol,
    pub children: HashMap<String, ExternalSymbol>,
    pub merged_ids: Vec<(ProviderId, luna_common::ids::SymbolId)>,
}

impl ExternalSymbol {
    pub fn new(sym: Symbol) -> Self {
        Self {
            sym,
            children: HashMap::new(),
            merged_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalSymbolId {
    pub provider_id: ProviderId,
    pub decl_id: Option<luna_ast::DeclId>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalImplKey {
    pub trait_id: Option<CanonicalSymbolId>,
    pub self_type_def: CanonicalSymbolId,
}

#[derive(Debug, Clone)]
pub struct ExternalTraitImplEntry {
    pub decl_id: Option<luna_ast::DeclId>,
    pub trait_id: CanonicalSymbolId,
    pub self_type: luna_semantic::ty::SemanticTypeId,
    pub generic_params: Vec<CanonicalSymbolId>,
}

#[derive(Debug, Clone)]
pub struct ProviderInterface {
    pub id: ProviderId,
    pub name: String,
    pub exported_symbols: HashMap<String, ExternalSymbol>,
    pub symbol_types: HashMap<luna_common::ids::SymbolId, luna_semantic::ty::SemanticTypeId>,
    pub types: luna_semantic::ty::TypeContext,
    pub lang_items:
        std::collections::HashMap<luna_semantic::lang_item::LangItem, CanonicalSymbolId>,
    pub generic_param_symbols: HashMap<CanonicalSymbolId, Vec<CanonicalSymbolId>>,
    pub trait_impls: HashMap<ExternalImplKey, Vec<luna_ast::DeclId>>,
    pub trait_impl_entries: Vec<ExternalTraitImplEntry>,
    pub impl_methods: HashMap<ExternalImplKey, Vec<CanonicalSymbolId>>,
    pub method_impls: HashMap<CanonicalSymbolId, ExternalImplKey>,
    pub impl_method_symbols: Vec<ExternalSymbol>,
    pub symbol_canonicals: HashMap<luna_common::ids::SymbolId, CanonicalSymbolId>,
    pub trait_associated_types: HashMap<CanonicalSymbolId, Vec<CanonicalSymbolId>>,
    pub impl_associated_types:
        HashMap<(ExternalImplKey, CanonicalSymbolId), luna_semantic::ty::SemanticTypeId>,
    pub impl_self_types: HashMap<ExternalImplKey, luna_semantic::ty::SemanticTypeId>,
    pub impl_generic_params: HashMap<ExternalImplKey, Vec<CanonicalSymbolId>>,
    pub impl_generic_param_symbols: Vec<ExternalSymbol>,
    pub trait_associated_type_symbols: Vec<ExternalSymbol>,
    pub internal_symbols: Vec<Symbol>,
    pub decl_symbols: HashMap<luna_ast::DeclId, luna_common::ids::SymbolId>,
    pub expr_symbols: HashMap<luna_ast::ExprId, luna_common::ids::SymbolId>,
    pub pat_symbols: HashMap<luna_ast::PatId, luna_common::ids::SymbolId>,
    pub pat_types: HashMap<luna_ast::PatId, luna_semantic::ty::SemanticTypeId>,
    pub expr_types: HashMap<luna_ast::ExprId, luna_semantic::ty::SemanticTypeId>,
    pub expr_substs: HashMap<luna_ast::ExprId, Vec<(luna_common::ids::SymbolId, luna_semantic::ty::SemanticTypeId)>>,
    pub expr_struct_init_indices: HashMap<luna_ast::ExprId, Vec<u32>>,
    pub raw_generic_param_symbols: HashMap<(luna_ast::DeclId, usize), luna_common::ids::SymbolId>,
    pub symbol_lifetime_contracts: HashMap<luna_common::ids::SymbolId, luna_semantic::CanonicalLifetimeContract>,
    pub trait_methods: HashMap<CanonicalSymbolId, Vec<CanonicalSymbolId>>,
}

#[derive(Default)]
pub struct ModuleRegistry {
    pub providers: HashMap<String, ProviderId>,
    pub local_providers: HashSet<String>,
    pub external_providers: HashSet<String>,
    pub interfaces: HashMap<ProviderId, ProviderInterface>,
    next_id: u32,
    loading_stack: Vec<String>,
}

impl ModuleRegistry {
    fn get_canonical(
        sym_id: luna_common::ids::SymbolId,
        ctx: &luna_semantic::SemanticContext,
        current_provider_id: luna_semantic::symbol::ProviderId,
    ) -> CanonicalSymbolId {
        let sym = ctx.symbol_table.get_symbol(sym_id);
        CanonicalSymbolId {
            provider_id: sym.provider_id.unwrap_or(current_provider_id),
            decl_id: sym.decl_id,
            name: sym.name.clone(),
        }
    }

    fn to_external_impl_key(
        key: &luna_semantic::semantic_tables::ImplKey,
        ctx: &luna_semantic::SemanticContext,
        current_provider_id: luna_semantic::symbol::ProviderId,
    ) -> ExternalImplKey {
        ExternalImplKey {
            trait_id: key
                .trait_id
                .map(|id| Self::get_canonical(id, ctx, current_provider_id)),
            self_type_def: Self::get_canonical(key.self_type_def, ctx, current_provider_id),
        }
    }

    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            local_providers: HashSet::new(),
            external_providers: HashSet::new(),
            interfaces: HashMap::new(),
            next_id: 1, // 0 is reserved for local
            loading_stack: Vec::new(),
        }
    }

    pub fn is_loading(&self, name: &str) -> bool {
        self.loading_stack.contains(&name.to_string())
    }

    pub fn start_loading(&mut self, name: &str) {
        self.loading_stack.push(name.to_string());
    }

    pub fn finish_loading(&mut self) {
        self.loading_stack.pop();
    }

    pub fn register(&mut self, name: String, interface: ProviderInterface) -> ProviderId {
        let id = interface.id;
        self.providers.insert(name, id);
        self.interfaces.insert(id, interface);
        id
    }

    pub fn register_local(&mut self, name: String, interface: ProviderInterface) -> ProviderId {
        self.local_providers.insert(name.clone());
        self.register(name, interface)
    }

    pub fn register_external(&mut self, name: String, interface: ProviderInterface) -> ProviderId {
        self.external_providers.insert(name.clone());
        self.register(name, interface)
    }

    pub fn allocate_id(&mut self) -> ProviderId {
        let id = ProviderId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn get_provider_name(&self, id: ProviderId) -> String {
        self.providers
            .iter()
            .find_map(|(k, &v)| if v == id { Some(k.clone()) } else { None })
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub fn inject_into_ctx(&self, ctx: &mut luna_semantic::SemanticContext) {
        let global_scope = luna_semantic::symbol::ScopeId(0);
        let mut provider_symbol_maps: HashMap<
            ProviderId,
            HashMap<luna_common::ids::SymbolId, luna_common::ids::SymbolId>,
        > = HashMap::new();

        // Inject all external providers into isolated provider scopes for `import <name>;`
        for (prov_name, &pid) in &self.providers {
            if let Some(prov_interface) = self.interfaces.get(&pid) {
                let prov_scope = ctx.symbol_table.create_scope(
                    luna_semantic::symbol::ScopeKind::Module,
                    Some(global_scope),
                );
                for root in prov_interface.exported_symbols.values() {
                    Self::inject_symbol(root, prov_scope, ctx, &mut provider_symbol_maps);
                }
                ctx.provider_scopes.insert(pid, prov_scope);
                ctx.provider_lookup.insert(prov_name.clone(), pid);
                ctx.external_module_scopes
                    .insert(prov_name.clone(), prov_scope);
            }
        }

        for interface in self.interfaces.values() {
            for ext_sym in &interface.impl_method_symbols {
                let dummy_scope = ctx.symbol_table.create_scope(
                    luna_semantic::symbol::ScopeKind::Struct,
                    Some(global_scope),
                );
                Self::inject_symbol(ext_sym, dummy_scope, ctx, &mut provider_symbol_maps);
            }
            for ext_sym in &interface.impl_generic_param_symbols {
                let dummy_scope = ctx.symbol_table.create_scope(
                    luna_semantic::symbol::ScopeKind::Struct,
                    Some(global_scope),
                );
                Self::inject_symbol(ext_sym, dummy_scope, ctx, &mut provider_symbol_maps);
            }
            for ext_sym in &interface.trait_associated_type_symbols {
                let dummy_scope = ctx.symbol_table.create_scope(
                    luna_semantic::symbol::ScopeKind::Struct,
                    Some(global_scope),
                );
                Self::inject_symbol(ext_sym, dummy_scope, ctx, &mut provider_symbol_maps);
            }
        }

        for interface in self.interfaces.values() {
            for sym in &interface.internal_symbols {
                if provider_symbol_maps
                    .get(&interface.id)
                    .map_or(false, |m| m.contains_key(&sym.id))
                {
                    continue;
                }
                let dummy_scope = ctx.symbol_table.create_scope(
                    luna_semantic::symbol::ScopeKind::Block,
                    Some(global_scope),
                );
                let mut dummy_diags = Vec::new();
                let new_sym_id = ctx.symbol_table.declare_symbol(
                    sym.name.clone(),
                    sym.kind,
                    dummy_scope,
                    sym.span,
                    sym.decl_id,
                    Visibility::Private,
                    &mut dummy_diags,
                );
                ctx.symbol_table.symbols[new_sym_id.0 as usize].provider_id = Some(interface.id);
                provider_symbol_maps
                    .entry(interface.id)
                    .or_default()
                    .insert(sym.id, new_sym_id);
                if let Some(decl_id) = sym.decl_id {
                    let is_sub_symbol = matches!(sym.kind, SymbolKind::EnumVariant(_) | SymbolKind::TypeParam);
                    if !is_sub_symbol {
                        ctx.tables.decl_symbols.insert(decl_id, new_sym_id);
                    }
                    ctx.tables.symbol_decls.insert(new_sym_id, decl_id);
                }
            }
        }

        let mut canonical_map = HashMap::new();
        let mut by_name_map = HashMap::new();
        for sym_id in 0..ctx.symbol_table.symbols.len() {
            let sym = &ctx.symbol_table.symbols[sym_id];
            let pid = sym
                .provider_id
                .unwrap_or(luna_semantic::symbol::ProviderId(0));
            let canonical = CanonicalSymbolId {
                provider_id: pid,
                decl_id: sym.decl_id,
                name: sym.name.clone(),
            };
            canonical_map.insert(canonical, luna_common::ids::SymbolId(sym_id as u32));
            by_name_map.insert((pid, sym.name.clone()), luna_common::ids::SymbolId(sym_id as u32));
        }

        let resolve_canonical =
            |canonical: &CanonicalSymbolId| -> Option<luna_common::ids::SymbolId> {
                canonical_map.get(canonical).copied().or_else(|| {
                    by_name_map.get(&(canonical.provider_id, canonical.name.clone())).copied()
                })
            };

        // Now inject types
        for interface in self.interfaces.values() {
            let lookup_sym = |sym: luna_common::ids::SymbolId| -> luna_common::ids::SymbolId {
                if let Some(&new_id) = provider_symbol_maps
                    .get(&interface.id)
                    .and_then(|m| m.get(&sym))
                {
                    return new_id;
                }
                if let Some(canon) = interface.symbol_canonicals.get(&sym) {
                    if let Some(new_id) = resolve_canonical(canon) {
                        return new_id;
                    }
                    for pid in self.providers.values() {
                        if let Some(&new_id) = by_name_map.get(&(*pid, canon.name.clone())) {
                            return new_id;
                        }
                    }
                }
                if let Some(&new_id) = provider_symbol_maps.values().find_map(|m| m.get(&sym)) {
                    return new_id;
                }
                sym
            };

            for (&old_sym_id, &old_ty_id) in &interface.symbol_types {
                if let Some(&new_sym_id) = provider_symbol_maps
                    .get(&interface.id)
                    .and_then(|m| m.get(&old_sym_id))
                {
                    let new_ty_id =
                        ctx.types
                            .clone_type_from(old_ty_id, &interface.types, &lookup_sym);
                    ctx.tables.symbol_types.insert(new_sym_id, new_ty_id);
                }
            }

            // Inject lang items
            for (item, canon) in &interface.lang_items {
                if let Some(sym_id) = resolve_canonical(canon) {
                    ctx.lang_items.inject_raw(*item, sym_id);
                }
            }

            // Inject lifetime contracts
            for (&old_sym_id, contract) in &interface.symbol_lifetime_contracts {
                let new_sym_id = lookup_sym(old_sym_id);
                ctx.tables.fn_lifetime_contracts.insert(new_sym_id, contract.clone());
            }

            // Inject generic param symbols
            for (old_canon_id, gp_list) in &interface.generic_param_symbols {
                if let Some(new_sym_id) = resolve_canonical(old_canon_id) {
                    if let Some(&decl_id) = ctx.tables.symbol_decls.get(&new_sym_id) {
                        for (idx, old_gp_canon) in gp_list.iter().enumerate() {
                            if let Some(new_gp_sym) = resolve_canonical(old_gp_canon) {
                                ctx.tables
                                    .generic_param_symbols
                                    .insert((decl_id, idx), new_gp_sym);
                            }
                        }
                    }
                }
            }

            // Inject trait impl entries with coherence checking
            for entry in &interface.trait_impl_entries {
                if let Some(trait_sym) = resolve_canonical(&entry.trait_id) {
                    let new_self_ty =
                        ctx.types
                            .clone_type_from(entry.self_type, &interface.types, &lookup_sym);
                    let new_gps: Vec<_> =
                        entry.generic_params.iter().filter_map(resolve_canonical).collect();
                    let dummy_span = luna_common::Span::default();
                    if ctx
                        .check_impl_coherence(trait_sym, new_self_ty, &new_gps, dummy_span)
                        .is_err()
                    {
                        continue;
                    }
                    ctx.tables
                        .trait_impl_entries
                        .push(luna_semantic::semantic_tables::TraitImplEntry {
                            decl_id: entry.decl_id,
                            trait_id: trait_sym,
                            self_type: new_self_ty,
                            generic_params: new_gps,
                        });
                }
            }

            // Inject trait impls
            for (old_impl_key, decl_ids) in &interface.trait_impls {
                let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                    let new_impl_key = luna_semantic::semantic_tables::ImplKey {
                        trait_id: new_trait_id,
                        self_type_def: new_self_type,
                    };
                    let entry = ctx
                        .tables
                        .trait_impls
                        .entry(new_impl_key)
                        .or_insert_with(Vec::new);
                    for &did in decl_ids {
                        if !entry.contains(&did) {
                            entry.push(did);
                        }
                    }
                }
            }

            // Inject impl methods
            for (old_impl_key, old_canon_ids) in &interface.impl_methods {
                let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                    let new_impl_key = luna_semantic::semantic_tables::ImplKey {
                        trait_id: new_trait_id,
                        self_type_def: new_self_type,
                    };
                    let new_sym_ids: Vec<_> =
                        old_canon_ids.iter().filter_map(resolve_canonical).collect();
                    ctx.tables.impl_methods.insert(new_impl_key, new_sym_ids);
                }
            }

            // Inject method impls
            for (old_canon_id, old_impl_key) in &interface.method_impls {
                if let Some(new_sym_id) = resolve_canonical(old_canon_id) {
                    let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                    if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                        let new_impl_key = luna_semantic::semantic_tables::ImplKey {
                            trait_id: new_trait_id,
                            self_type_def: new_self_type,
                        };
                        ctx.tables.method_impls.insert(new_sym_id, new_impl_key);
                    }
                }
            }

            // Inject trait associated types
            for (canon_trait, canon_assocs) in &interface.trait_associated_types {
                if let Some(trait_sym) = resolve_canonical(canon_trait) {
                    let mut assocs = Vec::new();
                    for a in canon_assocs {
                        if let Some(a_sym) = resolve_canonical(a) {
                            assocs.push(a_sym);
                            ctx.tables.assoc_type_traits.insert(a_sym, trait_sym);
                            let a_sym_name = ctx.symbol_table.get_symbol(a_sym).name.clone();
                            ctx.tables
                                .assoc_type_names
                                .insert((trait_sym, a_sym_name), a_sym);
                        }
                    }
                    ctx.tables.trait_associated_types.insert(trait_sym, assocs);
                }
            }

            // Inject trait methods
            for (canon_trait, canon_meths) in &interface.trait_methods {
                if let Some(trait_sym) = resolve_canonical(canon_trait) {
                    let mut meths = Vec::new();
                    for m in canon_meths {
                        if let Some(m_sym) = resolve_canonical(m) {
                            meths.push(m_sym);
                        }
                    }
                    ctx.tables.trait_methods.insert(trait_sym, meths);
                }
            }

            // Inject impl associated types
            for ((old_impl_key, old_assoc_canon), &old_ty_id) in &interface.impl_associated_types {
                let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                    if let Some(new_assoc_sym) = resolve_canonical(old_assoc_canon) {
                        let new_impl_key = luna_semantic::semantic_tables::ImplKey {
                            trait_id: new_trait_id,
                            self_type_def: new_self_type,
                        };
                        let new_ty_id =
                            ctx.types
                                .clone_type_from(old_ty_id, &interface.types, &lookup_sym);
                        ctx.tables
                            .impl_associated_types
                            .insert((new_impl_key, new_assoc_sym), new_ty_id);
                    }
                }
            }

            // Inject impl self types
            for (old_impl_key, &old_self_ty) in &interface.impl_self_types {
                let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                    let new_impl_key = luna_semantic::semantic_tables::ImplKey {
                        trait_id: new_trait_id,
                        self_type_def: new_self_type,
                    };
                    let new_self_ty =
                        ctx.types
                            .clone_type_from(old_self_ty, &interface.types, &lookup_sym);
                    ctx.tables.impl_self_types.insert(new_impl_key, new_self_ty);
                }
            }

            // Inject impl generic params
            for (old_impl_key, canon_gps) in &interface.impl_generic_params {
                let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                    let new_impl_key = luna_semantic::semantic_tables::ImplKey {
                        trait_id: new_trait_id,
                        self_type_def: new_self_type,
                    };
                    let new_gps: Vec<_> = canon_gps.iter().filter_map(resolve_canonical).collect();
                    ctx.tables.impl_generic_params.insert(new_impl_key, new_gps);
                }
            }

            // Inject lang items
            for (lang_item, canon_id) in &interface.lang_items {
                if let Some(new_sym_id) = resolve_canonical(canon_id) {
                    ctx.lang_items.inject_raw(*lang_item, new_sym_id);
                }
            }

            // Inject AST tables for generic function monomorphization
            for (&decl_id, &old_sym_id) in &interface.decl_symbols {
                let new_sym_id = lookup_sym(old_sym_id);
                ctx.tables.decl_symbols.insert(decl_id, new_sym_id);
                ctx.tables.symbol_decls.insert(new_sym_id, decl_id);
            }
            for (&expr_id, &old_sym_id) in &interface.expr_symbols {
                let new_sym_id = lookup_sym(old_sym_id);
                ctx.tables.expr_symbols.insert(expr_id, new_sym_id);
            }
            for (&pat_id, &old_sym_id) in &interface.pat_symbols {
                let new_sym_id = lookup_sym(old_sym_id);
                ctx.tables.pat_symbols.insert(pat_id, new_sym_id);
            }
            for (&pat_id, &old_ty_id) in &interface.pat_types {
                let new_ty_id = ctx.types.clone_type_from(old_ty_id, &interface.types, &lookup_sym);
                ctx.tables.pat_types.insert(pat_id, new_ty_id);
            }
            for (&expr_id, &old_ty_id) in &interface.expr_types {
                let new_ty_id = ctx.types.clone_type_from(old_ty_id, &interface.types, &lookup_sym);
                ctx.tables.expr_types.insert(expr_id, new_ty_id);
            }
            for (&expr_id, subst_list) in &interface.expr_substs {
                let mut new_subst = luna_semantic::ty::Substitution::new();
                for &(old_gp, old_ty) in subst_list {
                    let new_gp = lookup_sym(old_gp);
                    let new_ty = ctx.types.clone_type_from(old_ty, &interface.types, &lookup_sym);
                    new_subst.insert(new_gp, new_ty);
                }
                ctx.tables.expr_substs.insert(expr_id, new_subst);
            }
            for (&expr_id, indices) in &interface.expr_struct_init_indices {
                ctx.tables.expr_struct_init_indices.insert(expr_id, indices.clone());
            }
            for (&(decl_id, idx), &old_gp) in &interface.raw_generic_param_symbols {
                let new_gp = lookup_sym(old_gp);
                ctx.tables.generic_param_symbols.insert((decl_id, idx), new_gp);
            }
        }
    }

    fn merge_symbols(
        target: &mut ExternalSymbol,
        source: &ExternalSymbol,
    ) {
        if let Some(pid) = source.sym.provider_id {
            target.merged_ids.push((pid, source.sym.id));
        }
        for (name, child) in &source.children {
            if let Some(existing) = target.children.get_mut(name) {
                Self::merge_symbols(existing, child);
            } else {
                target.children.insert(name.clone(), child.clone());
            }
        }
    }

    fn inject_symbol(
        ext_sym: &ExternalSymbol,
        parent_scope: luna_semantic::symbol::ScopeId,
        ctx: &mut luna_semantic::SemanticContext,
        provider_symbol_maps: &mut HashMap<
            ProviderId,
            HashMap<luna_common::ids::SymbolId, luna_common::ids::SymbolId>,
        >,
    ) {
        let sym = &ext_sym.sym;

        let new_sym_id = ctx.symbol_table.declare_symbol(
            sym.name.clone(),
            sym.kind,
            parent_scope,
            sym.span,
            sym.decl_id,
            sym.visibility,
            &mut ctx.diagnostics,
        );

        let pid = sym
            .provider_id
            .unwrap_or(luna_semantic::symbol::ProviderId(0));
        provider_symbol_maps
            .entry(pid)
            .or_default()
            .insert(sym.id, new_sym_id);
        for &(mpid, mid) in &ext_sym.merged_ids {
            provider_symbol_maps
                .entry(mpid)
                .or_default()
                .insert(mid, new_sym_id);
        }

        if let Some(decl_id) = sym.decl_id {
            let is_sub_symbol =
                matches!(sym.kind, SymbolKind::EnumVariant(_) | SymbolKind::TypeParam);
            if !is_sub_symbol {
                ctx.tables.decl_symbols.insert(decl_id, new_sym_id);
            }
            ctx.tables.symbol_decls.insert(new_sym_id, decl_id);
            if sym.kind == SymbolKind::Macro {
                ctx.tables.macro_decls.insert(new_sym_id, decl_id);
            }
        }

        ctx.symbol_table.symbols[new_sym_id.0 as usize].provider_id = sym.provider_id;

        if !ext_sym.children.is_empty()
            || sym.kind == SymbolKind::Module
            || sym.kind == SymbolKind::Enum
            || sym.kind == SymbolKind::Struct
        {
            let scope_kind = match sym.kind {
                SymbolKind::Module => ScopeKind::Module,
                SymbolKind::Enum | SymbolKind::Struct | SymbolKind::Trait => ScopeKind::Struct,
                _ => ScopeKind::Module,
            };

            let inner_scope = ctx
                .symbol_table
                .create_scope(scope_kind, Some(parent_scope));
            ctx.symbol_table.set_inner_scope(new_sym_id, inner_scope);
            if let Some(did) = sym.decl_id {
                ctx.tables.decl_scopes.insert(did, inner_scope);
            }

            for child in ext_sym.children.values() {
                Self::inject_symbol(child, inner_scope, ctx, provider_symbol_maps);
            }
        }
    }

    pub fn extract_interface_from_ctx(
        provider_name: String,
        provider_id: ProviderId,
        ctx: &luna_semantic::SemanticContext,
    ) -> ProviderInterface {
        let mut exported_symbols = HashMap::new();
        let global_scope = luna_semantic::symbol::ScopeId(0);
        let mut visited = HashSet::new();
        let mut symbol_types = HashMap::new();

        Self::extract_scope(
            &mut exported_symbols,
            global_scope,
            ctx,
            provider_id,
            &mut visited,
            &mut symbol_types,
        );

        let mut generic_param_symbols = HashMap::new();
        for (&(decl_id, idx), &gp_sym) in &ctx.tables.generic_param_symbols {
            if let Some(&sym_id) = ctx.tables.decl_symbols.get(&decl_id) {
                generic_param_symbols
                    .entry(sym_id)
                    .or_insert_with(Vec::new)
                    .push((idx, gp_sym));
            }
        }
        let mut final_generic_param_symbols = HashMap::new();
        for (sym_id, mut list) in generic_param_symbols {
            list.sort_by_key(|&(idx, _)| idx);
            final_generic_param_symbols.insert(
                Self::get_canonical(sym_id, ctx, provider_id),
                list.into_iter()
                    .map(|(_, gp_sym)| Self::get_canonical(gp_sym, ctx, provider_id))
                    .collect(),
            );
        }
        let mut impl_method_symbols = Vec::new();
        for method_syms in ctx.tables.impl_methods.values() {
            for &sym_id in method_syms {
                let mut sym = ctx.symbol_table.get_symbol(sym_id).clone();
                if sym.provider_id.is_some() {
                    continue;
                }
                sym.provider_id = Some(provider_id);
                let mut ext_sym = ExternalSymbol::new(sym.clone());
                let inner_scope = sym.inner_scope.or_else(|| {
                    sym.decl_id
                        .and_then(|did| ctx.tables.decl_scopes.get(&did).copied())
                });
                if let Some(inner) = inner_scope {
                    Self::extract_scope(
                        &mut ext_sym.children,
                        inner,
                        ctx,
                        provider_id,
                        &mut visited,
                        &mut symbol_types,
                    );
                }
                if let Some(ty_id) = ctx.tables.symbol_types.get(&sym_id) {
                    symbol_types.insert(sym_id, *ty_id);
                }
                impl_method_symbols.push(ext_sym);
            }
        }

        let mut trait_impls = HashMap::new();
        for (k, v) in &ctx.tables.trait_impls {
            let ext_key = Self::to_external_impl_key(k, ctx, provider_id);
            if ext_key
                .trait_id
                .as_ref()
                .map_or(false, |t| t.provider_id == provider_id)
                || ext_key.self_type_def.provider_id == provider_id
            {
                trait_impls.insert(ext_key, v.clone());
            }
        }

        let mut impl_methods = HashMap::new();
        for (k, v) in &ctx.tables.impl_methods {
            let ext_key = Self::to_external_impl_key(k, ctx, provider_id);
            if ext_key
                .trait_id
                .as_ref()
                .map_or(false, |t| t.provider_id == provider_id)
                || ext_key.self_type_def.provider_id == provider_id
            {
                impl_methods.insert(
                    ext_key,
                    v.iter()
                        .map(|&id| Self::get_canonical(id, ctx, provider_id))
                        .collect(),
                );
            }
        }

        let mut method_impls = HashMap::new();
        for (k, v) in &ctx.tables.method_impls {
            let canon_k = Self::get_canonical(*k, ctx, provider_id);
            if canon_k.provider_id == provider_id {
                method_impls.insert(canon_k, Self::to_external_impl_key(v, ctx, provider_id));
            }
        }

        let mut lang_items = HashMap::new();
        for (item, sym_id) in ctx.lang_items.iter() {
            let canon = Self::get_canonical(sym_id, ctx, provider_id);
            lang_items.insert(item, canon);
        }

        let mut symbol_canonicals = HashMap::new();
        for sym_id in 0..ctx.symbol_table.symbols.len() {
            let sid = luna_common::ids::SymbolId(sym_id as u32);
            symbol_canonicals.insert(sid, Self::get_canonical(sid, ctx, provider_id));
        }

        let mut trait_associated_types = HashMap::new();
        let mut trait_associated_type_symbols = Vec::new();
        for (&trait_sym, assoc_syms) in &ctx.tables.trait_associated_types {
            let canon_trait = Self::get_canonical(trait_sym, ctx, provider_id);
            if canon_trait.provider_id == provider_id {
                let mut canon_assocs = Vec::new();
                for &s in assoc_syms {
                    let mut sym = ctx.symbol_table.get_symbol(s).clone();
                    if sym.provider_id.is_none() {
                        sym.provider_id = Some(provider_id);
                    }
                    trait_associated_type_symbols.push(ExternalSymbol::new(sym));
                    canon_assocs.push(Self::get_canonical(s, ctx, provider_id));
                }
                trait_associated_types.insert(canon_trait, canon_assocs);
            }
        }

        let mut trait_methods = HashMap::new();
        for (&trait_sym, meth_syms) in &ctx.tables.trait_methods {
            let canon_trait = Self::get_canonical(trait_sym, ctx, provider_id);
            if canon_trait.provider_id == provider_id {
                let mut canon_meths = Vec::new();
                for &s in meth_syms {
                    canon_meths.push(Self::get_canonical(s, ctx, provider_id));
                }
                trait_methods.insert(canon_trait, canon_meths);
            }
        }

        let mut impl_associated_types = HashMap::new();
        for ((impl_key, trait_assoc_sym), &ty_id) in &ctx.tables.impl_associated_types {
            let ext_key = Self::to_external_impl_key(impl_key, ctx, provider_id);
            let canon_assoc = Self::get_canonical(*trait_assoc_sym, ctx, provider_id);
            if ext_key
                .trait_id
                .as_ref()
                .map_or(false, |t| t.provider_id == provider_id)
                || ext_key.self_type_def.provider_id == provider_id
            {
                impl_associated_types.insert((ext_key, canon_assoc), ty_id);
            }
        }

        let mut impl_generic_param_symbols = Vec::new();
        for gp_syms in ctx.tables.impl_generic_params.values() {
            for &sym_id in gp_syms {
                let mut sym = ctx.symbol_table.get_symbol(sym_id).clone();
                if sym.provider_id.is_some() {
                    continue;
                }
                sym.provider_id = Some(provider_id);
                let ext_sym = ExternalSymbol::new(sym);
                impl_generic_param_symbols.push(ext_sym);
            }
        }

        let mut impl_generic_params = HashMap::new();
        for (impl_key, gp_syms) in &ctx.tables.impl_generic_params {
            let ext_key = Self::to_external_impl_key(impl_key, ctx, provider_id);
            if ext_key
                .trait_id
                .as_ref()
                .map_or(false, |t| t.provider_id == provider_id)
                || ext_key.self_type_def.provider_id == provider_id
            {
                let canon_gps = gp_syms
                    .iter()
                    .map(|&s| Self::get_canonical(s, ctx, provider_id))
                    .collect();
                impl_generic_params.insert(ext_key, canon_gps);
            }
        }

        let mut impl_self_types = HashMap::new();
        for (impl_key, &self_ty_id) in &ctx.tables.impl_self_types {
            let ext_key = Self::to_external_impl_key(impl_key, ctx, provider_id);
            if ext_key
                .trait_id
                .as_ref()
                .map_or(false, |t| t.provider_id == provider_id)
                || ext_key.self_type_def.provider_id == provider_id
            {
                impl_self_types.insert(ext_key, self_ty_id);
            }
        }

        let mut trait_impl_entries = Vec::new();
        for entry in &ctx.tables.trait_impl_entries {
            let canon_trait = Self::get_canonical(entry.trait_id, ctx, provider_id);
            let head_sym = ctx.nominal_head(entry.self_type);
            let head_canon = head_sym.map(|s| Self::get_canonical(s, ctx, provider_id));
            let is_local_trait = canon_trait.provider_id == provider_id;
            let is_local_head = head_canon.as_ref().map_or(false, |h| h.provider_id == provider_id);
            if is_local_trait || is_local_head {
                let canon_gps = entry
                    .generic_params
                    .iter()
                    .map(|&s| Self::get_canonical(s, ctx, provider_id))
                    .collect();
                trait_impl_entries.push(ExternalTraitImplEntry {
                    decl_id: entry.decl_id,
                    trait_id: canon_trait,
                    self_type: entry.self_type,
                    generic_params: canon_gps,
                });
            }
        }

        let mut internal_symbols = Vec::new();
        for sym in &ctx.symbol_table.symbols {
            if sym.provider_id == Some(provider_id) || sym.provider_id.is_none() {
                internal_symbols.push(sym.clone());
            }
        }
        for (&sym_id, &ty_id) in &ctx.tables.symbol_types {
            symbol_types.insert(sym_id, ty_id);
        }
        let decl_symbols = ctx.tables.decl_symbols.clone();
        let expr_symbols = ctx.tables.expr_symbols.clone();
        let pat_symbols = ctx.tables.pat_symbols.clone();
        let pat_types = ctx.tables.pat_types.clone();
        let expr_types = ctx.tables.expr_types.clone();
        let expr_struct_init_indices = ctx.tables.expr_struct_init_indices.clone();
        let raw_generic_param_symbols = ctx.tables.generic_param_symbols.clone();
        let mut expr_substs = HashMap::new();
        for (&eid, subst) in &ctx.tables.expr_substs {
            let list: Vec<_> = subst.map.iter().map(|(&s, &t)| (s, t)).collect();
            expr_substs.insert(eid, list);
        }

        ProviderInterface {
            id: provider_id,
            name: provider_name,
            exported_symbols,
            symbol_types,
            types: ctx.types.clone(),
            lang_items,
            generic_param_symbols: final_generic_param_symbols,
            trait_impls,
            trait_impl_entries,
            impl_methods,
            method_impls,
            impl_method_symbols,
            symbol_canonicals,
            trait_associated_types,
            impl_associated_types,
            impl_self_types,
            impl_generic_params,
            impl_generic_param_symbols,
            trait_associated_type_symbols,
            internal_symbols,
            decl_symbols,
            expr_symbols,
            pat_symbols,
            pat_types,
            expr_types,
            expr_substs,
            expr_struct_init_indices,
            raw_generic_param_symbols,
            symbol_lifetime_contracts: ctx.tables.fn_lifetime_contracts.clone(),
            trait_methods,
        }
    }

    fn extract_scope(
        target: &mut HashMap<String, ExternalSymbol>,
        scope_id: luna_semantic::symbol::ScopeId,
        ctx: &luna_semantic::SemanticContext,
        provider_id: ProviderId,
        visited: &mut HashSet<luna_semantic::symbol::ScopeId>,
        symbol_types: &mut HashMap<
            luna_common::ids::SymbolId,
            luna_semantic::ty::SemanticTypeId,
        >,
    ) {
        if !visited.insert(scope_id) {
            return; // Cycle detected
        }
        let scope = &ctx.symbol_table.scopes[scope_id.0 as usize];
        for (_name, sym_ids) in &scope.symbols {
            if let Some(&sym_id) = sym_ids.last() {
                let mut sym = ctx.symbol_table.get_symbol(sym_id).clone();
                if sym.provider_id.is_some() && sym.provider_id != Some(provider_id) {
                    if sym.kind == SymbolKind::Module {
                        let inner_scope = sym.inner_scope.or_else(|| {
                            sym.decl_id
                                .and_then(|did| ctx.tables.decl_scopes.get(&did).copied())
                        });
                        if let Some(inner) = inner_scope {
                            let mut children = HashMap::new();
                            Self::extract_scope(
                                &mut children,
                                inner,
                                ctx,
                                provider_id,
                                visited,
                                symbol_types,
                            );
                            if !children.is_empty() {
                                let mut ext_sym = ExternalSymbol::new(sym.clone());
                                ext_sym.sym.provider_id = Some(provider_id);
                                ext_sym.children = children;
                                target.insert(sym.name.clone(), ext_sym);
                            }
                        }
                    }
                    continue;
                }
                // Only extract Public symbols, TypeParams, or Module namespaces (and fields within a struct)
                if scope.kind != ScopeKind::Struct
                    && sym.visibility != Visibility::Public
                    && sym.kind != SymbolKind::TypeParam
                    && sym.kind != SymbolKind::Module
                {
                    continue;
                }
                // Tag the symbol with the provider ID
                sym.provider_id = Some(provider_id);

                let mut ext_sym = ExternalSymbol::new(sym.clone());

                let inner_scope = sym.inner_scope.or_else(|| {
                    sym.decl_id
                        .and_then(|did| ctx.tables.decl_scopes.get(&did).copied())
                });

                if let Some(inner) = inner_scope {
                    Self::extract_scope(
                        &mut ext_sym.children,
                        inner,
                        ctx,
                        provider_id,
                        visited,
                        symbol_types,
                    );
                }

                // If this is a non-public module and it didn't export any children for this provider, skip it.
                if sym.kind == SymbolKind::Module
                    && sym.visibility != Visibility::Public
                    && ext_sym.children.is_empty()
                {
                    continue;
                }

                if let Some(ty_id) = ctx.tables.symbol_types.get(&sym_id) {
                    symbol_types.insert(sym_id, *ty_id);
                } else {
                }

                if let Some(existing) = target.get_mut(&sym.name) {
                    Self::merge_symbols(existing, &ext_sym);
                } else {
                    target.insert(sym.name.clone(), ext_sym);
                }
            }
        }
    }
}
