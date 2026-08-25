pub mod resolver;
pub mod typechecker;
pub mod symbol;
pub mod semantic_tables;
pub mod ty;
pub mod mono;

pub use resolver::Resolver;
pub use typechecker::TypeChecker;
pub use mono::{Monomorphizer, MonoInstance};
pub use symbol::{SymbolTable, ScopeId, SymbolKind};
pub use mellis_common::ids::SymbolId;

pub use semantic_tables::SemanticTables;
pub use ty::{TypeContext, SemanticTypeId, SemanticType, BuiltinType};

use mellis_ast::AstArena;
use mellis_common::Diagnostic;

use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NeedsDropState {
    Visiting,
    Yes,
    No,
}

pub struct SemanticContext {
    pub symbol_table: SymbolTable,
    pub tables: SemanticTables,
    pub types: TypeContext,
    pub mono_instances: Vec<MonoInstance>,
    pub diagnostics: Vec<Diagnostic>,
    pub needs_drop_cache: RefCell<HashMap<ty::SemanticTypeId, NeedsDropState>>,
}

impl SemanticContext {
    pub fn new() -> Self {
        Self {
            symbol_table: SymbolTable::new(),
            tables: SemanticTables::new(),
            types: TypeContext::new(),
            mono_instances: Vec::new(),
            diagnostics: Vec::new(),
            needs_drop_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn get_symbol_type(&self, sym_id: SymbolId) -> Option<&SemanticType> {
        let ty_id = self.tables.symbol_types.get(&sym_id)?;
        Some(self.types.get(*ty_id))
    }

    pub fn needs_drop(&self, id: ty::SemanticTypeId) -> bool {
        if let Some(&state) = self.needs_drop_cache.borrow().get(&id) {
            match state {
                NeedsDropState::Yes => return true,
                NeedsDropState::No => return false,
                NeedsDropState::Visiting => return false, // break cycle safely
            }
        }
        
        self.needs_drop_cache.borrow_mut().insert(id, NeedsDropState::Visiting);
        
        let ty = self.types.get(id);
        let result = match ty {
            ty::SemanticType::Struct(sym_id, fields) => {
                if self.tables.drop_impls.contains(sym_id) {
                    true
                } else {
                    fields.iter().any(|&f| self.needs_drop(f))
                }
            }
            ty::SemanticType::Enum(sym_id, variants) => {
                if self.tables.drop_impls.contains(sym_id) {
                    true
                } else {
                    variants.iter().any(|&v| self.needs_drop(v))
                }
            }
            ty::SemanticType::Tuple(fields) => {
                fields.iter().any(|&f| self.needs_drop(f))
            }
            ty::SemanticType::Array(elem_ty, _) => {
                self.needs_drop(*elem_ty)
            }
            _ => false,
        };
        
        self.needs_drop_cache.borrow_mut().insert(id, if result { NeedsDropState::Yes } else { NeedsDropState::No });
        result
    }
}
