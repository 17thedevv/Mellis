use mellis_ast::{ExprId, StmtId, DeclId, PatId, TypeId as AstTypeId};
use mellis_common::ids::SymbolId;
use std::collections::HashMap;

use crate::ty::SemanticTypeId;

pub struct SemanticTables {
    pub expr_types: HashMap<ExprId, SemanticTypeId>,
    pub expr_symbols: HashMap<ExprId, SymbolId>,
    
    pub pat_symbols: HashMap<PatId, SymbolId>,
    pub pat_types: HashMap<PatId, SemanticTypeId>,
    
    pub decl_symbols: HashMap<DeclId, SymbolId>,
    pub symbol_decls: HashMap<SymbolId, DeclId>,
    
    
    pub ast_type_to_semantic: HashMap<AstTypeId, SemanticTypeId>,
    pub symbol_types: HashMap<SymbolId, SemanticTypeId>,
}

impl SemanticTables {
    pub fn new() -> Self {
        Self {
            expr_types: HashMap::new(),
            expr_symbols: HashMap::new(),
            pat_symbols: HashMap::new(),
            pat_types: HashMap::new(),
            decl_symbols: HashMap::new(),
            symbol_decls: HashMap::new(),
            ast_type_to_semantic: HashMap::new(),
            symbol_types: HashMap::new(),
        }
    }
}
