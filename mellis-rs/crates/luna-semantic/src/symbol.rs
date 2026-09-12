use luna_common::ids::{Span, SymbolId, SyntaxContext};
use luna_ast::{DeclId, Visibility};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProviderId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImportSymbolResult {
    Success,
    AlreadyImported,
    Conflict { existing: SymbolId },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    ExternFunction,
    Variable,
    Constant,
    Struct,
    Enum,
    EnumVariant(u32),
    Trait,
    TraitMethod,
    Alias,
    AssociatedType,
    Module,
    Type,
    TypeParam,
    LifetimeParam,
    Macro,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IdentKey {
    pub name: String,
    pub ctxt: SyntaxContext,
}

impl IdentKey {
    pub fn new(name: impl Into<String>, ctxt: SyntaxContext) -> Self {
        Self {
            name: name.into(),
            ctxt,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub ctxt: SyntaxContext,
    pub kind: SymbolKind,
    pub scope: ScopeId,
    pub span: Span,
    pub visibility: Visibility,
    pub decl_id: Option<DeclId>,
    pub inner_scope: Option<ScopeId>,
    pub provider_id: Option<ProviderId>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Module,
    Function,
    Block,
    Struct,
    TypeAlias,
    GenericParam,
}

#[derive(Clone, Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub symbols: HashMap<IdentKey, Vec<SymbolId>>,
}

pub struct SymbolTable {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
}

impl SymbolTable {
    pub fn new() -> Self {
        let mut table = Self {
            scopes: Vec::new(),
            symbols: Vec::new(),
        };
        // Create global scope at index 0
        table.create_scope(ScopeKind::Global, None);
        table
    }

    pub fn create_scope(&mut self, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope {
            id,
            kind,
            parent,
            symbols: HashMap::new(),
        });
        id
    }

    pub fn is_ancestor(&self, ancestor: ScopeId, descendant: ScopeId) -> bool {
        let mut current = Some(descendant);
        while let Some(scope_id) = current {
            if scope_id == ancestor {
                return true;
            }
            current = self.scopes[scope_id.0 as usize].parent;
        }
        false
    }

    pub fn declare_symbol(
        &mut self,
        name: String,
        kind: SymbolKind,
        scope_id: ScopeId,
        span: Span,
        decl_id: Option<DeclId>,
        visibility: Visibility,
        diagnostics: &mut Vec<luna_common::Diagnostic>,
    ) -> SymbolId {
        let key = IdentKey::new(name.clone(), span.ctxt);

        // Check for duplicate in the same scope with matching syntax context
        if let Some(syms) = self.scopes[scope_id.0 as usize].symbols.get(&key) {
            if let Some(&existing_sym_id) = syms.last() {
                let existing_sym = &self.symbols[existing_sym_id.0 as usize];
                if existing_sym.span != span {
                    diagnostics.push(
                        luna_common::Diagnostic::error(format!(
                            "Duplicate definition of symbol `{}` in the same scope",
                            name
                        ))
                        .with_span(span),
                    );
                }
                // Return the existing symbol to avoid breaking downstream
                return existing_sym_id;
            }
        }

        let id = SymbolId(self.symbols.len() as u32);
        let symbol = Symbol {
            id,
            name,
            ctxt: span.ctxt,
            kind,
            scope: scope_id,
            span,
            visibility,
            decl_id,
            inner_scope: None,
            provider_id: None,
        };
        self.symbols.push(symbol);

        // Add to scope
        self.scopes[scope_id.0 as usize]
            .symbols
            .entry(key)
            .or_insert_with(Vec::new)
            .push(id);

        id
    }

    pub fn set_inner_scope(&mut self, symbol_id: SymbolId, inner_scope: ScopeId) {
        self.symbols[symbol_id.0 as usize].inner_scope = Some(inner_scope);
    }

    pub fn add_symbol_to_scope(&mut self, name: String, symbol_id: SymbolId, scope_id: ScopeId) {
        let sym = &self.symbols[symbol_id.0 as usize];
        let key = IdentKey::new(name, sym.ctxt);
        self.scopes[scope_id.0 as usize]
            .symbols
            .entry(key)
            .or_insert_with(Vec::new)
            .push(symbol_id);
    }

    pub fn add_imported_symbol(
        &mut self,
        name: String,
        sym_id: SymbolId,
        scope_id: ScopeId,
    ) -> ImportSymbolResult {
        let sym = &self.symbols[sym_id.0 as usize];
        let key = IdentKey::new(name, sym.ctxt);
        let sym_kind = sym.kind;
        let sym_provider_id = sym.provider_id;
        let sym_decl_id = sym.decl_id;
        let sym_name = sym.name.clone();
        let sym_inner_scope = sym.inner_scope;

        let existing_sym_id = self.scopes[scope_id.0 as usize]
            .symbols
            .get(&key)
            .and_then(|syms| syms.last().copied());

        if let Some(existing_sym_id) = existing_sym_id {
            let existing = &self.symbols[existing_sym_id.0 as usize];
            // 1. Same canonical symbol ID
            if existing_sym_id == sym_id {
                return ImportSymbolResult::AlreadyImported;
            }
            // 2. Same provider and same declaration identity or name
            if existing.provider_id.is_some()
                && existing.provider_id == sym_provider_id
                && (existing.decl_id == sym_decl_id || existing.name == sym_name)
            {
                return ImportSymbolResult::AlreadyImported;
            }
            // Module namespace aggregation: multiple providers can contribute to the same module namespace
            if existing.kind == SymbolKind::Module && sym_kind == SymbolKind::Module {
                let existing_inner = match self.symbols[existing_sym_id.0 as usize].inner_scope {
                    Some(s) => s,
                    None => {
                        let s = self.create_scope(ScopeKind::Module, Some(scope_id));
                        self.symbols[existing_sym_id.0 as usize].inner_scope = Some(s);
                        s
                    }
                };
                if let Some(src_scope) = sym_inner_scope {
                    let children: Vec<(String, SymbolId)> = {
                        let s = &self.scopes[src_scope.0 as usize];
                        let mut res = Vec::new();
                        for (k, v) in &s.symbols {
                            if let Some(&id) = v.last() {
                                res.push((k.name.clone(), id));
                            }
                        }
                        res
                    };
                    for (child_name, child_id) in children {
                        let res = self.add_imported_symbol(child_name, child_id, existing_inner);
                        if let ImportSymbolResult::Conflict { existing } = res {
                            return ImportSymbolResult::Conflict { existing };
                        }
                    }
                }
                return ImportSymbolResult::Success;
            }
            // 3. Different provider or conflicting local definition
            return ImportSymbolResult::Conflict {
                existing: existing_sym_id,
            };
        }

        self.scopes[scope_id.0 as usize]
            .symbols
            .entry(key)
            .or_insert_with(Vec::new)
            .push(sym_id);
        ImportSymbolResult::Success
    }

    pub fn enclosing_module_scope(&self, mut scope_id: ScopeId) -> ScopeId {
        loop {
            let scope = &self.scopes[scope_id.0 as usize];
            if matches!(scope.kind, ScopeKind::Module | ScopeKind::Global) {
                return scope_id;
            }
            if let Some(parent) = scope.parent {
                scope_id = parent;
            } else {
                return scope_id;
            }
        }
    }

    pub fn is_accessible(&self, sym_id: SymbolId, current_scope: ScopeId, current_provider: Option<ProviderId>) -> bool {
        let sym = &self.symbols[sym_id.0 as usize];
        match sym.visibility {
            luna_ast::Visibility::Public => true,
            luna_ast::Visibility::Internal => sym.provider_id == current_provider,
            luna_ast::Visibility::Private => {
                if sym.provider_id.is_some() && sym.provider_id != current_provider {
                    return false;
                }
                let sym_mod = self.enclosing_module_scope(sym.scope);
                let cur_mod = self.enclosing_module_scope(current_scope);
                sym_mod == cur_mod || self.is_ancestor(sym_mod, cur_mod)
            }
        }
    }

    pub fn is_symbol_foreign(&self, sym_id: SymbolId, current_provider: Option<ProviderId>) -> bool {
        let sym = &self.symbols[sym_id.0 as usize];
        sym.provider_id.is_some() && sym.provider_id != current_provider
    }

    pub fn is_symbol_local(&self, sym_id: SymbolId, current_provider: Option<ProviderId>) -> bool {
        !self.is_symbol_foreign(sym_id, current_provider)
    }

    pub fn lookup(&self, name: &str, start_scope: ScopeId) -> Option<SymbolId> {
        self.lookup_with_ctxt(name, SyntaxContext::ROOT, start_scope)
    }

    pub fn lookup_with_ctxt(&self, name: &str, ctxt: SyntaxContext, start_scope: ScopeId) -> Option<SymbolId> {
        let target_key = IdentKey::new(name, ctxt);
        let root_key = IdentKey::new(name, SyntaxContext::ROOT);

        let mut current = Some(start_scope);
        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0 as usize];
            if let Some(syms) = scope.symbols.get(&target_key) {
                if let Some(&last_sym) = syms.last() {
                    return Some(last_sym);
                }
            }

            // Transparent fallback for non-root contexts to find global/module items
            if !ctxt.is_root() && matches!(scope.kind, ScopeKind::Global | ScopeKind::Module) {
                if let Some(syms) = scope.symbols.get(&root_key) {
                    if let Some(&last_sym) = syms.last() {
                        return Some(last_sym);
                    }
                }
            }

            current = scope.parent;
        }
        None
    }

    pub fn lookup_exact(&self, name: &str, scope_id: ScopeId) -> Option<SymbolId> {
        self.lookup_exact_with_ctxt(name, SyntaxContext::ROOT, scope_id)
    }

    pub fn lookup_exact_with_ctxt(&self, name: &str, ctxt: SyntaxContext, scope_id: ScopeId) -> Option<SymbolId> {
        let scope = &self.scopes[scope_id.0 as usize];
        let target_key = IdentKey::new(name, ctxt);
        if let Some(syms) = scope.symbols.get(&target_key) {
            if let Some(&last_sym) = syms.last() {
                return Some(last_sym);
            }
        }
        if !ctxt.is_root() && matches!(scope.kind, ScopeKind::Global | ScopeKind::Module) {
            let root_key = IdentKey::new(name, SyntaxContext::ROOT);
            if let Some(syms) = scope.symbols.get(&root_key) {
                if let Some(&last_sym) = syms.last() {
                    return Some(last_sym);
                }
            }
        }
        None
    }

    pub fn lookup_macro(&self, path: &[&str], start_scope: ScopeId) -> Option<SymbolId> {
        self.lookup_macro_with_ctxt(path, SyntaxContext::ROOT, start_scope)
    }

    pub fn lookup_macro_with_ctxt(&self, path: &[&str], ctxt: SyntaxContext, start_scope: ScopeId) -> Option<SymbolId> {
        if path.is_empty() {
            return None;
        }
        if path.len() == 1 {
            if let Some(sym_id) = self.lookup_with_ctxt(path[0], ctxt, start_scope) {
                if matches!(self.symbols[sym_id.0 as usize].kind, SymbolKind::Macro) {
                    return Some(sym_id);
                }
            }
            return None;
        }

        // Qualified path: e.g. ["module", "macro_name"] or ["alias", "macro_name"]
        let first_seg = path[0];
        let mut current_scope = if let Some(sym_id) = self.lookup_with_ctxt(first_seg, ctxt, start_scope) {
            let sym = &self.symbols[sym_id.0 as usize];
            sym.inner_scope?
        } else {
            return None;
        };

        for &seg in &path[1..path.len() - 1] {
            if let Some(sym_id) = self.lookup_exact_with_ctxt(seg, ctxt, current_scope) {
                let sym = &self.symbols[sym_id.0 as usize];
                if let Some(inner) = sym.inner_scope {
                    current_scope = inner;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        let last_seg = path[path.len() - 1];
        if let Some(sym_id) = self.lookup_exact_with_ctxt(last_seg, ctxt, current_scope) {
            if matches!(self.symbols[sym_id.0 as usize].kind, SymbolKind::Macro) {
                return Some(sym_id);
            }
        }
        None
    }
    
    pub fn get_module_scope(&self, mut child: ScopeId) -> ScopeId {
        loop {
            let scope = &self.scopes[child.0 as usize];
            if matches!(scope.kind, ScopeKind::Global | ScopeKind::Module) {
                return child;
            }
            if let Some(parent) = scope.parent {
                child = parent;
            } else {
                return child;
            }
        }
    }

    pub fn is_descendant(&self, mut child: ScopeId, ancestor: ScopeId) -> bool {
        loop {
            if child == ancestor {
                return true;
            }
            if let Some(p) = self.scopes[child.0 as usize].parent {
                child = p;
            } else {
                return false;
            }
        }
    }

    pub fn get_symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    pub fn get_symbol_mut(&mut self, id: SymbolId) -> &mut Symbol {
        &mut self.symbols[id.0 as usize]
    }
}
