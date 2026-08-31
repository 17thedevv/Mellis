use std::collections::{HashMap, HashSet};
use mellis_semantic::symbol::{Symbol, SymbolKind, ScopeKind, ProviderId};
use mellis_ast::Visibility;
use mellis_ast::AstArena;
use mellis_common::Diagnostic;

#[derive(Debug, Clone)]
pub struct ExternalSymbol {
    pub sym: Symbol,
    pub children: HashMap<String, ExternalSymbol>,
}

impl ExternalSymbol {
    pub fn new(sym: Symbol) -> Self {
        Self {
            sym,
            children: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderInterface {
    pub id: ProviderId,
    pub name: String,
    pub exported_symbols: HashMap<String, ExternalSymbol>,
    pub symbol_types: HashMap<mellis_common::ids::SymbolId, mellis_semantic::ty::SemanticTypeId>,
    pub types: mellis_semantic::ty::TypeContext,
}

pub struct ModuleRegistry {
    pub providers: HashMap<String, ProviderId>,
    pub interfaces: HashMap<ProviderId, ProviderInterface>,
    next_id: u32,
    loading_stack: Vec<String>,
}

impl ModuleRegistry {
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
        
        for interface in self.interfaces.values() {
            for (name, root) in &interface.exported_symbols {
                if let Some(existing) = global_roots.get_mut(name) {
                    Self::merge_symbols(existing, root, ctx);
                } else {
                    global_roots.insert(name.clone(), root.clone());
                }
            }
        }
        
        let global_scope = mellis_semantic::symbol::ScopeId(0);
        let mut symbol_map = HashMap::new();
        
        for root in global_roots.values() {
            Self::inject_symbol(root, global_scope, ctx, &mut symbol_map);
        }
        
        // Now inject types
        for interface in self.interfaces.values() {
            for (&old_sym_id, &old_ty_id) in &interface.symbol_types {
                if let Some(&new_sym_id) = symbol_map.get(&old_sym_id) {
                    let new_ty_id = ctx.types.clone_type_from(old_ty_id, &interface.types, &symbol_map);
                    ctx.tables.symbol_types.insert(new_sym_id, new_ty_id);
                }
            }
        }
    }
    
    fn merge_symbols(target: &mut ExternalSymbol, source: &ExternalSymbol, ctx: &mut mellis_semantic::SemanticContext) {
        for (name, child) in &source.children {
            if let Some(existing) = target.children.get_mut(name) {
                Self::merge_symbols(existing, child, ctx);
            } else {
                target.children.insert(name.clone(), child.clone());
            }
        }
    }
    
    fn inject_symbol(ext_sym: &ExternalSymbol, parent_scope: mellis_semantic::symbol::ScopeId, ctx: &mut mellis_semantic::SemanticContext, symbol_map: &mut HashMap<mellis_common::ids::SymbolId, mellis_common::ids::SymbolId>) {
        let sym = &ext_sym.sym;
        
        println!("inject_symbol: injecting '{}' of kind {:?} into scope {}", sym.name, sym.kind, parent_scope.0);
        
        let new_sym_id = ctx.symbol_table.declare_symbol(
            sym.name.clone(),
            sym.kind,
            parent_scope,
            sym.span,
            sym.decl_id,
            Visibility::Public,
            &mut ctx.diagnostics
        );
        
        symbol_map.insert(sym.id, new_sym_id);
        
        if let Some(decl_id) = sym.decl_id {
            ctx.tables.decl_symbols.insert(decl_id, new_sym_id);
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
                Self::inject_symbol(child, inner_scope, ctx, symbol_map);
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

        ProviderInterface {
            id: provider_id,
            name: provider_name,
            exported_symbols,
            symbol_types,
            types: ctx.types.clone(),
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
                // Only extract Public symbols
                if sym.visibility != Visibility::Public {
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
                }
                
                target.insert(sym.name.clone(), ext_sym);
            }
        }
    }
}
