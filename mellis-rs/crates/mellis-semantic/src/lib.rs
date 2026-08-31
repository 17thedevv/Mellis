pub mod resolver;
pub mod typechecker;
pub mod symbol;
pub mod semantic_tables;
pub mod ty;
pub mod mono;
pub mod macro_engine;
pub mod annotation;
pub mod derive;
pub mod comptime;
pub mod effect;

pub use effect::{Effect, EffectSet};
pub use resolver::Resolver;
pub use resolver::{ModuleNamespaceProvider, ModuleNamespaceMap};
pub use typechecker::TypeChecker;
pub use mono::{MonoCollector, MonoInstance, InstantiatedFunction};
pub use macro_engine::MacroEngine;
pub use annotation::AttributeProcessor;
pub use derive::{DeriveRegistry, DeriveContext, DeriveInput, DeriveKind};
pub use comptime::{ComptimeValue, ComptimeEvaluator, ComptimeContext, ComptimeError, IntWidth, FloatWidth};
pub use symbol::{SymbolTable, ScopeId, SymbolKind};
pub use mellis_common::ids::SymbolId;

pub use semantic_tables::{CaptureBinding, CaptureMode, SemanticTables};
pub use ty::{TypeContext, SemanticTypeId, SemanticType, BuiltinType};

pub trait ComptimeEngine: Send + Sync {
    fn eval_expr(&self, arena: &mellis_ast::AstArena, ctx: &SemanticContext, source: &str, expr_id: mellis_ast::ExprId) -> Result<ComptimeValue, ComptimeError>;
    fn eval_stmt(&self, arena: &mellis_ast::AstArena, ctx: &SemanticContext, source: &str, stmt_id: mellis_ast::StmtId) -> Result<ComptimeValue, ComptimeError>;
}

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
    pub instantiated_functions: Vec<InstantiatedFunction>,
    pub diagnostics: Vec<Diagnostic>,
    pub needs_drop_cache: RefCell<HashMap<ty::SemanticTypeId, NeedsDropState>>,
    pub comptime_values: HashMap<mellis_ast::ExprId, comptime::ComptimeValue>,
    pub const_values: HashMap<SymbolId, comptime::ComptimeValue>,
}

impl SemanticContext {
    pub fn new() -> Self {
        Self {
            symbol_table: SymbolTable::new(),
            tables: SemanticTables::new(),
            types: TypeContext::new(),
            instantiated_functions: Vec::new(),
            diagnostics: Vec::new(),
            needs_drop_cache: RefCell::new(HashMap::new()),
            comptime_values: HashMap::new(),
            const_values: HashMap::new(),
        }
    }

    pub fn get_symbol_type(&self, sym_id: SymbolId) -> Option<&SemanticType> {
        let ty_id = self.tables.symbol_types.get(&sym_id)?;
        Some(self.types.get(*ty_id))
    }

    pub fn needs_drop(&self, id: ty::SemanticTypeId) -> bool {
        let ty = self.types.get(id);
        if let ty::SemanticType::Struct(sym_id, _) = ty {
            let name = &self.symbol_table.get_symbol(*sym_id).name;
            if name == "File" {
                println!("DEBUG: needs_drop checking File, drop_impls.contains_key: {}", self.tables.drop_impls.contains_key(sym_id));
            }
        }
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
                if self.tables.drop_impls.contains_key(sym_id) {
                    true
                } else {
                    fields.iter().any(|&f| self.needs_drop(f))
                }
            }
            ty::SemanticType::Enum(sym_id, variants) => {
                if self.tables.drop_impls.contains_key(&sym_id) {
                    true
                } else {
                    variants.iter().any(|&v| self.needs_drop(v))
                }
            }
            ty::SemanticType::Box(_) => true,

            ty::SemanticType::Tuple(fields) => {
                fields.iter().any(|&f| self.needs_drop(f))
            }
            ty::SemanticType::Closure(_, _, env_ty) => {
                self.needs_drop(*env_ty)
            }
            ty::SemanticType::Array(elem_ty, _) => {
                self.needs_drop(*elem_ty)
            }
            ty::SemanticType::Future(inner) => {
                self.needs_drop(*inner)
            }
            _ => false,
        };
        
        self.needs_drop_cache.borrow_mut().insert(id, if result { NeedsDropState::Yes } else { NeedsDropState::No });
        result
    }
}
