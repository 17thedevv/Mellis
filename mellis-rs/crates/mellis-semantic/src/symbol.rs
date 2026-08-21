use mellis_common::ids::{Span, SymbolId};
use mellis_ast::{DeclId, Visibility};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Variable,
    Constant,
    Struct,
    Enum,
    EnumVariant(u32),
    Trait,
    TraitMethod,
    Alias,
    Module,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub scope: ScopeId,
    pub span: Span,
    pub visibility: Visibility,
    pub decl_id: Option<DeclId>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Module,
    Function,
    Block,
}

#[derive(Clone, Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub symbols: HashMap<String, Vec<SymbolId>>,
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

    pub fn declare_symbol(
        &mut self,
        name: String,
        kind: SymbolKind,
        scope_id: ScopeId,
        span: Span,
        decl_id: Option<DeclId>,
        visibility: Visibility,
    ) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        let symbol = Symbol {
            id,
            name: name.clone(),
            kind,
            scope: scope_id,
            span,
            visibility,
            decl_id,
        };
        self.symbols.push(symbol);

        // Add to scope
        self.scopes[scope_id.0 as usize]
            .symbols
            .entry(name)
            .or_insert_with(Vec::new)
            .push(id);

        id
    }

    pub fn lookup(&self, name: &str, start_scope: ScopeId) -> Option<SymbolId> {
        let mut current = Some(start_scope);
        while let Some(scope_id) = current {
            let scope = &self.scopes[scope_id.0 as usize];
            if let Some(syms) = scope.symbols.get(name) {
                if let Some(&last_sym) = syms.last() {
                    return Some(last_sym);
                }
            }
            current = scope.parent;
        }
        None
    }

    pub fn get_symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }
}
