use mellis_ast::{ExprId, StmtId, DeclId, PatId, TypeId as AstTypeId};
use mellis_common::ids::SymbolId;
use std::collections::{HashMap, HashSet};

use crate::ty::SemanticTypeId;
use crate::ScopeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureMode {
    SharedBorrow,
    MutableBorrow,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureBinding {
    pub symbol: SymbolId,
    pub mode: CaptureMode,
    pub env_field: u32,
    pub ty: SemanticTypeId,
    pub env_ty: SemanticTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImplKey {
    pub trait_id: Option<SymbolId>,
    pub self_type_def: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitBound {
    pub param: SymbolId,
    pub trait_id: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitResolution {
    pub trait_id: SymbolId,
    pub method_sym: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntrinsicKind {
    BoxNew,
    Null,
    Cast,
    PtrOffset,
    PtrWrite,
    SizeOf,
    AlignOf,
    TypeOf,
    TypeInfo,
}

pub struct SemanticTables {
    pub expr_types: HashMap<ExprId, SemanticTypeId>,
    pub expr_symbols: HashMap<ExprId, SymbolId>,
    pub expr_substs: HashMap<ExprId, crate::ty::Substitution>,
    pub expr_trait_resolutions: HashMap<ExprId, TraitResolution>,
    pub intrinsic_types: HashMap<ExprId, SemanticTypeId>,
    pub expr_intrinsics: HashMap<ExprId, IntrinsicKind>,
    pub expr_member_indices: HashMap<ExprId, u32>,
    pub expr_struct_init_indices: HashMap<ExprId, Vec<u32>>,
    pub expr_sizeof_target: HashMap<ExprId, SemanticTypeId>,
    pub expr_lifetimes: HashMap<ExprId, crate::ty::LifetimeId>,
    pub expr_captures: HashMap<ExprId, Vec<SymbolId>>,
    pub closure_capture_bindings: HashMap<ExprId, Vec<CaptureBinding>>,
    pub closure_mutated_captures: HashMap<ExprId, HashSet<SymbolId>>,
    pub closure_env_types: HashMap<ExprId, SemanticTypeId>,
    pub closure_env_ptr_types: HashMap<ExprId, SemanticTypeId>,
    
    // Dynamic dispatch tables
    pub dyn_method_indices: HashMap<ExprId, u32>,
    pub dyn_coercions: HashMap<ExprId, (SymbolId, SymbolId)>,
    
    // For loop desugaring tracking
    pub for_loop_next: HashMap<StmtId, SymbolId>,
    pub for_loop_subst: HashMap<StmtId, crate::ty::Substitution>,
    
    pub pat_symbols: HashMap<PatId, SymbolId>,
    pub pat_types: HashMap<PatId, SemanticTypeId>,
    
    pub decl_symbols: HashMap<DeclId, SymbolId>,
    pub symbol_decls: HashMap<SymbolId, DeclId>,
    
    pub type_symbols: HashMap<AstTypeId, SymbolId>,
    
    pub ast_type_to_semantic: HashMap<AstTypeId, SemanticTypeId>,
    pub symbol_types: HashMap<SymbolId, SemanticTypeId>,
    pub type_scopes: HashMap<AstTypeId, ScopeId>,
    pub decl_scopes: HashMap<DeclId, ScopeId>,
    
    // Maps a function's SymbolId to a boolean vector indicating which parameters are @sync_noescape
    pub ffi_sync_noescape: HashMap<SymbolId, Vec<bool>>,
    
    // Maps a (DeclId, param_index) to the SymbolId of the generic parameter
    pub generic_param_symbols: HashMap<(DeclId, usize), SymbolId>,
    
    // Structs that implement Drop -> the drop function's SymbolId
    pub drop_impls: HashMap<SymbolId, SymbolId>,
    
    // Canonical impl resolution: ImplKey -> Impl DeclId
    pub trait_impls: HashMap<ImplKey, Vec<DeclId>>,
    
    // Maps a Trait's SymbolId to its required method SymbolIds
    pub trait_methods: HashMap<SymbolId, Vec<SymbolId>>,
    
    // Maps a Struct's SymbolId to its field SymbolIds
    pub struct_fields: HashMap<SymbolId, Vec<SymbolId>>,
    
    // Maps a GenericParam's SymbolId to its TraitBounds
    pub trait_bounds: HashMap<SymbolId, Vec<TraitBound>>,
    
    // Maps a base struct/enum SymbolId to a list of its method SymbolIds (from inherent impl blocks without trait)
    pub impl_methods: HashMap<ImplKey, Vec<SymbolId>>,
    
    // Maps a method SymbolId to its parent impl block DeclId
    pub method_impls: HashMap<SymbolId, ImplKey>,
    
    pub macro_decls: HashMap<SymbolId, DeclId>,
    pub decl_macros: HashMap<DeclId, SymbolId>,
    pub function_effects: HashMap<SymbolId, crate::effect::EffectSet>,
}

impl SemanticTables {

    pub fn expect_closure_capture_bindings(&self, id: ExprId) -> Vec<CaptureBinding> {
        self.closure_capture_bindings.get(&id).cloned().unwrap_or_else(|| {
            panic!("ICE: closure capture bindings missing for expr {:?}", id)
        })
    }

    pub fn new() -> Self {
        Self {
            expr_types: HashMap::new(),
            expr_symbols: HashMap::new(),
            expr_substs: HashMap::new(),
            expr_trait_resolutions: HashMap::new(),
            intrinsic_types: HashMap::new(),
            expr_intrinsics: HashMap::new(),
            expr_member_indices: HashMap::new(),
            expr_struct_init_indices: HashMap::new(),
            expr_sizeof_target: HashMap::new(),
            expr_lifetimes: HashMap::new(),
            expr_captures: HashMap::new(),
            closure_capture_bindings: HashMap::new(),
            closure_mutated_captures: HashMap::new(),
            closure_env_types: HashMap::new(),
            closure_env_ptr_types: HashMap::new(),
            dyn_method_indices: HashMap::new(),
            dyn_coercions: HashMap::new(),
            for_loop_next: HashMap::new(),
            for_loop_subst: HashMap::new(),
            pat_symbols: HashMap::new(),
            pat_types: HashMap::new(),
            decl_symbols: HashMap::new(),
            symbol_decls: HashMap::new(),
            type_symbols: HashMap::new(),
            ast_type_to_semantic: HashMap::new(),
            symbol_types: HashMap::new(),
            type_scopes: HashMap::new(),
            decl_scopes: HashMap::new(),
            ffi_sync_noescape: HashMap::new(),
            drop_impls: HashMap::new(),
            generic_param_symbols: HashMap::new(),
            trait_impls: HashMap::new(),
            trait_methods: HashMap::new(),
            struct_fields: HashMap::new(),
            trait_bounds: HashMap::new(),
            impl_methods: HashMap::new(),
            method_impls: HashMap::new(),
            macro_decls: HashMap::new(),
            decl_macros: HashMap::new(),
            function_effects: HashMap::new(),
        }
    }
}
