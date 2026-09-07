use std::collections::{HashMap, HashSet};
use mellis_semantic::symbol::{Symbol, SymbolKind, ScopeKind, ProviderId};
use mellis_ast::Visibility;
use mellis_ast::AstArena;
use mellis_common::Diagnostic;

#[derive(Debug, Clone)]
pub struct ExternalSymbol {
    pub sym: Symbol,
    pub children: HashMap<String, ExternalSymbol>,
    pub merged_ids: Vec<(ProviderId, mellis_common::ids::SymbolId)>,
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
    pub decl_id: Option<mellis_ast::DeclId>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalImplKey {
    pub trait_id: Option<CanonicalSymbolId>,
    pub self_type_def: CanonicalSymbolId,
}

#[derive(Debug, Clone)]
pub struct ProviderInterface {
    pub id: ProviderId,
    pub name: String,
    pub exported_symbols: HashMap<String, ExternalSymbol>,
    pub symbol_types: HashMap<mellis_common::ids::SymbolId, mellis_semantic::ty::SemanticTypeId>,
    pub types: mellis_semantic::ty::TypeContext,
    pub lang_items: std::collections::HashMap<mellis_semantic::lang_item::LangItem, CanonicalSymbolId>,
    pub generic_param_symbols: HashMap<CanonicalSymbolId, Vec<CanonicalSymbolId>>,
    pub trait_impls: HashMap<ExternalImplKey, Vec<mellis_ast::DeclId>>,
    pub impl_methods: HashMap<ExternalImplKey, Vec<CanonicalSymbolId>>,
    pub method_impls: HashMap<CanonicalSymbolId, ExternalImplKey>,
    pub impl_method_symbols: Vec<ExternalSymbol>,
    pub symbol_canonicals: HashMap<mellis_common::ids::SymbolId, CanonicalSymbolId>,
}

pub struct ModuleRegistry {
    pub providers: HashMap<String, ProviderId>,
    pub interfaces: HashMap<ProviderId, ProviderInterface>,
    next_id: u32,
    loading_stack: Vec<String>,
}

impl ModuleRegistry {

    fn get_canonical(sym_id: mellis_common::ids::SymbolId, ctx: &mellis_semantic::SemanticContext, current_provider_id: mellis_semantic::symbol::ProviderId) -> CanonicalSymbolId {
        let sym = ctx.symbol_table.get_symbol(sym_id);
        CanonicalSymbolId {
            provider_id: sym.provider_id.unwrap_or(current_provider_id),
            decl_id: sym.decl_id,
            name: sym.name.clone(),
        }
    }

    fn to_external_impl_key(key: &mellis_semantic::semantic_tables::ImplKey, ctx: &mellis_semantic::SemanticContext, current_provider_id: mellis_semantic::symbol::ProviderId) -> ExternalImplKey {
        ExternalImplKey {
            trait_id: key.trait_id.map(|id| Self::get_canonical(id, ctx, current_provider_id)),
            self_type_def: Self::get_canonical(key.self_type_def, ctx, current_provider_id),
        }
    }

    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
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
    
    pub fn allocate_id(&mut self) -> ProviderId {
        let id = ProviderId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn inject_into_ctx(&self, ctx: &mut mellis_semantic::SemanticContext) {
        let mut global_roots: HashMap<String, ExternalSymbol> = HashMap::new();
        
        let core_id = self.providers.get("core").copied();

        for (&provider_id, interface) in &self.interfaces {
            if Some(provider_id) == core_id {
                continue; // Do NOT leak core symbols into global_roots!
            }
            for (name, root) in &interface.exported_symbols {
                if let Some(existing) = global_roots.get_mut(name) {
                    Self::merge_symbols(existing, root, ctx);
                } else {
                    global_roots.insert(name.clone(), root.clone());
                }
            }
        }
        
        let global_scope = mellis_semantic::symbol::ScopeId(0);
        let mut provider_symbol_maps: HashMap<ProviderId, HashMap<mellis_common::ids::SymbolId, mellis_common::ids::SymbolId>> = HashMap::new();
        
        for root in global_roots.values() {
            Self::inject_symbol(root, global_scope, ctx, &mut provider_symbol_maps);
        }

        // Inject core symbols into an isolated module scope (never into global_scope)
        if let Some(cid) = core_id {
            if let Some(core_interface) = self.interfaces.get(&cid) {
                let core_scope = ctx.symbol_table.create_scope(mellis_semantic::symbol::ScopeKind::Module, Some(global_scope));
                for root in core_interface.exported_symbols.values() {
                    Self::inject_symbol(root, core_scope, ctx, &mut provider_symbol_maps);
                }
                ctx.external_module_scopes.insert("core".to_string(), core_scope);
            }
        }

        for interface in self.interfaces.values() {
            for ext_sym in &interface.impl_method_symbols {
                let dummy_scope = ctx.symbol_table.create_scope(mellis_semantic::symbol::ScopeKind::Struct, Some(global_scope));
                Self::inject_symbol(ext_sym, dummy_scope, ctx, &mut provider_symbol_maps);
            }
        }
        

        let mut canonical_map = HashMap::new();
        for sym_id in 0..ctx.symbol_table.symbols.len() {
            let sym = &ctx.symbol_table.symbols[sym_id];
            let canonical = CanonicalSymbolId {
                provider_id: sym.provider_id.unwrap_or(mellis_semantic::symbol::ProviderId(0)),
                decl_id: sym.decl_id,
                name: sym.name.clone(),
            };
            if sym.name == "branch" || sym.name == "from_output" {
                println!("DEBUG: canonical_map adding {:?} -> {:?}", canonical, sym_id);
            }
            canonical_map.insert(canonical, mellis_common::ids::SymbolId(sym_id as u32));
        }

        let resolve_canonical = |canonical: &CanonicalSymbolId| -> Option<mellis_common::ids::SymbolId> {
            canonical_map.get(canonical).copied()
        };

        // Now inject types
        for interface in self.interfaces.values() {
            let lookup_sym = |sym: mellis_common::ids::SymbolId| -> mellis_common::ids::SymbolId {
                if let Some(&new_id) = provider_symbol_maps.get(&interface.id).and_then(|m| m.get(&sym)) {
                    return new_id;
                }
                if let Some(canon) = interface.symbol_canonicals.get(&sym) {
                    if let Some(new_id) = resolve_canonical(canon) {
                        return new_id;
                    }
                }
                if let Some(&new_id) = provider_symbol_maps.values().find_map(|m| m.get(&sym)) {
                    return new_id;
                }
                sym
            };

            for (&old_sym_id, &old_ty_id) in &interface.symbol_types {
                if let Some(&new_sym_id) = provider_symbol_maps.get(&interface.id).and_then(|m| m.get(&old_sym_id)) {
                    let new_ty_id = ctx.types.clone_type_from(old_ty_id, &interface.types, &lookup_sym);
                    ctx.tables.symbol_types.insert(new_sym_id, new_ty_id);
                }
            }
            
            // Inject lang items
            for (item, canon) in &interface.lang_items {
                if let Some(sym_id) = resolve_canonical(canon) {
                    ctx.lang_items.inject_raw(*item, sym_id);
                }
            }

            // Inject generic param symbols
            for (old_canon_id, gp_list) in &interface.generic_param_symbols {
                if let Some(new_sym_id) = resolve_canonical(old_canon_id) {
                    if let Some(&decl_id) = ctx.tables.symbol_decls.get(&new_sym_id) {
                        for (idx, old_gp_canon) in gp_list.iter().enumerate() {
                            if let Some(new_gp_sym) = resolve_canonical(old_gp_canon) {
                                ctx.tables.generic_param_symbols.insert((decl_id, idx), new_gp_sym);
                            }
                        }
                    }
                }
            }

            // Inject trait impls
            for (old_impl_key, decl_ids) in &interface.trait_impls {
                let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                    let new_impl_key = mellis_semantic::semantic_tables::ImplKey {
                        trait_id: new_trait_id,
                        self_type_def: new_self_type,
                    };
                    ctx.tables.trait_impls.insert(new_impl_key, decl_ids.clone());
                }
            }
            
            // Inject impl methods
            for (old_impl_key, old_canon_ids) in &interface.impl_methods {
                let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                    let new_impl_key = mellis_semantic::semantic_tables::ImplKey {
                        trait_id: new_trait_id,
                        self_type_def: new_self_type,
                    };
                    let new_sym_ids: Vec<_> = old_canon_ids.iter().filter_map(resolve_canonical).collect();
                    ctx.tables.impl_methods.insert(new_impl_key, new_sym_ids);
                }
            }
            
            // Inject method impls
            for (old_canon_id, old_impl_key) in &interface.method_impls {
                if let Some(new_sym_id) = resolve_canonical(old_canon_id) {
                    let new_trait_id = old_impl_key.trait_id.as_ref().and_then(resolve_canonical);
                    if let Some(new_self_type) = resolve_canonical(&old_impl_key.self_type_def) {
                        let new_impl_key = mellis_semantic::semantic_tables::ImplKey {
                            trait_id: new_trait_id,
                            self_type_def: new_self_type,
                        };
                        ctx.tables.method_impls.insert(new_sym_id, new_impl_key);
                    }
                }
            }
            
            // Inject lang items
            for (lang_item, canon_id) in &interface.lang_items {
                if let Some(new_sym_id) = resolve_canonical(canon_id) {
                    println!("DEBUG: Injecting lang item {:?} -> sym_id {:?}", lang_item, new_sym_id);
                    ctx.lang_items.inject_raw(*lang_item, new_sym_id);
                } else {
                    println!("DEBUG: Failed to resolve lang item {:?} (canon_id: {:?})", lang_item, canon_id);
                }
            }
        }
    }
    
    fn merge_symbols(target: &mut ExternalSymbol, source: &ExternalSymbol, ctx: &mut mellis_semantic::SemanticContext) {
        if let Some(pid) = source.sym.provider_id {
            target.merged_ids.push((pid, source.sym.id));
        }
        for (name, child) in &source.children {
            if let Some(existing) = target.children.get_mut(name) {
                Self::merge_symbols(existing, child, ctx);
            } else {
                target.children.insert(name.clone(), child.clone());
            }
        }
    }
    
    fn inject_symbol(
        ext_sym: &ExternalSymbol,
        parent_scope: mellis_semantic::symbol::ScopeId,
        ctx: &mut mellis_semantic::SemanticContext,
        provider_symbol_maps: &mut HashMap<ProviderId, HashMap<mellis_common::ids::SymbolId, mellis_common::ids::SymbolId>>,
    ) {
        let sym = &ext_sym.sym;
        
        let new_sym_id = ctx.symbol_table.declare_symbol(
            sym.name.clone(),
            sym.kind,
            parent_scope,
            sym.span,
            sym.decl_id,
            Visibility::Public,
            &mut ctx.diagnostics
        );
        
        let pid = sym.provider_id.unwrap_or(mellis_semantic::symbol::ProviderId(0));
        provider_symbol_maps.entry(pid).or_default().insert(sym.id, new_sym_id);
        for &(mpid, mid) in &ext_sym.merged_ids {
            provider_symbol_maps.entry(mpid).or_default().insert(mid, new_sym_id);
        }
        
        if let Some(decl_id) = sym.decl_id {
            let is_sub_symbol = matches!(sym.kind, SymbolKind::EnumVariant(_) | SymbolKind::TypeParam);
            if !is_sub_symbol {
                ctx.tables.decl_symbols.insert(decl_id, new_sym_id);
            }
            ctx.tables.symbol_decls.insert(new_sym_id, decl_id);
            if sym.kind == SymbolKind::Macro {
                ctx.tables.macro_decls.insert(new_sym_id, decl_id);
            }
        }
        
        ctx.symbol_table.symbols[new_sym_id.0 as usize].provider_id = sym.provider_id;
        
        if !ext_sym.children.is_empty() || sym.kind == SymbolKind::Module || sym.kind == SymbolKind::Enum || sym.kind == SymbolKind::Struct {
            let scope_kind = match sym.kind {
                SymbolKind::Module => ScopeKind::Module,
                SymbolKind::Enum | SymbolKind::Struct | SymbolKind::Trait => ScopeKind::Struct,
                _ => ScopeKind::Module,
            };
            
            let inner_scope = ctx.symbol_table.create_scope(scope_kind, Some(parent_scope));
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
        ctx: &mellis_semantic::SemanticContext,
    ) -> ProviderInterface {
        let mut exported_symbols = HashMap::new();
        let global_scope = mellis_semantic::symbol::ScopeId(0);
        let mut visited = HashSet::new();
        let mut symbol_types = HashMap::new();

        Self::extract_scope(&mut exported_symbols, global_scope, ctx, provider_id, &mut visited, &mut symbol_types);

        let mut generic_param_symbols = HashMap::new();
        for (&(decl_id, idx), &gp_sym) in &ctx.tables.generic_param_symbols {
            if let Some(&sym_id) = ctx.tables.decl_symbols.get(&decl_id) {
                generic_param_symbols.entry(sym_id).or_insert_with(Vec::new).push((idx, gp_sym));
            }
        }
                let mut final_generic_param_symbols = HashMap::new();
        for (sym_id, mut list) in generic_param_symbols {
            list.sort_by_key(|&(idx, _)| idx);
            final_generic_param_symbols.insert(Self::get_canonical(sym_id, ctx, provider_id), list.into_iter().map(|(_, gp_sym)| Self::get_canonical(gp_sym, ctx, provider_id)).collect());
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
                    sym.decl_id.and_then(|did| ctx.tables.decl_scopes.get(&did).copied())
                });
                if let Some(inner) = inner_scope {
                    Self::extract_scope(&mut ext_sym.children, inner, ctx, provider_id, &mut visited, &mut symbol_types);
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
            if ext_key.trait_id.as_ref().map_or(false, |t| t.provider_id == provider_id) || ext_key.self_type_def.provider_id == provider_id {
                trait_impls.insert(ext_key, v.clone());
            }
        }

        let mut impl_methods = HashMap::new();
        for (k, v) in &ctx.tables.impl_methods {
            let ext_key = Self::to_external_impl_key(k, ctx, provider_id);
            if ext_key.trait_id.as_ref().map_or(false, |t| t.provider_id == provider_id) || ext_key.self_type_def.provider_id == provider_id {
                impl_methods.insert(ext_key, v.iter().map(|&id| Self::get_canonical(id, ctx, provider_id)).collect());
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
            println!("DEBUG: Extracted LangItem {:?} -> Canon {:?}", item, canon);
            lang_items.insert(item, canon);
        }

        let mut symbol_canonicals = HashMap::new();
        for sym_id in 0..ctx.symbol_table.symbols.len() {
            let sid = mellis_common::ids::SymbolId(sym_id as u32);
            symbol_canonicals.insert(sid, Self::get_canonical(sid, ctx, provider_id));
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
            impl_methods,
            method_impls,
            impl_method_symbols,
            symbol_canonicals,
        }
    }

    fn extract_scope(
        target: &mut HashMap<String, ExternalSymbol>,
        scope_id: mellis_semantic::symbol::ScopeId,
        ctx: &mellis_semantic::SemanticContext,
        provider_id: ProviderId,
        visited: &mut HashSet<mellis_semantic::symbol::ScopeId>,
        symbol_types: &mut HashMap<mellis_common::ids::SymbolId, mellis_semantic::ty::SemanticTypeId>,
    ) {
        if !visited.insert(scope_id) {
            return; // Cycle detected
        }
        let scope = &ctx.symbol_table.scopes[scope_id.0 as usize];
        for (name, sym_ids) in &scope.symbols {
            if let Some(&sym_id) = sym_ids.last() {
                let mut sym = ctx.symbol_table.get_symbol(sym_id).clone();
                if sym.provider_id.is_some() {
                    continue;
                }
                // Only extract Public symbols or TypeParams
                if sym.visibility != Visibility::Public && sym.kind != SymbolKind::TypeParam {
                    continue;
                }
                // Tag the symbol with the provider ID
                sym.provider_id = Some(provider_id);

                let mut ext_sym = ExternalSymbol::new(sym.clone());
                
                let inner_scope = sym.inner_scope.or_else(|| {
                    sym.decl_id.and_then(|did| ctx.tables.decl_scopes.get(&did).copied())
                });
                
                if let Some(inner) = inner_scope {
                    Self::extract_scope(&mut ext_sym.children, inner, ctx, provider_id, visited, symbol_types);
                }
                
                if let Some(ty_id) = ctx.tables.symbol_types.get(&sym_id) {
                    symbol_types.insert(sym_id, *ty_id);
                } else {
                }
                
                target.insert(sym.name.clone(), ext_sym);
            }
        }
    }
}
