use luna_ast::{ExprId, StmtId, DeclId, PatId, TypeId as AstTypeId};
use luna_common::ids::SymbolId;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplEntry {
    pub decl_id: Option<DeclId>,
    pub trait_id: SymbolId,
    pub self_type: SemanticTypeId,
    pub generic_params: Vec<SymbolId>,
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
    pub try_branch_methods: HashMap<ExprId, luna_ast::DeclId>,
    pub try_from_residual_methods: HashMap<ExprId, luna_ast::DeclId>,
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
    
    // Dynamic dispatch and coercion tables
    pub dyn_method_indices: HashMap<ExprId, u32>,
    pub coercions: HashMap<ExprId, crate::coercion::CoercionKind>,
    
    // Try operator branches: (inner_success_idx, inner_failure_idx, func_failure_idx)
    pub try_branches: HashMap<ExprId, (u32, u32, u32)>,
    
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
    
    // Maps a Trait's SymbolId to its declared associated type SymbolIds
    pub trait_associated_types: HashMap<SymbolId, Vec<SymbolId>>,
    // Maps an associated type SymbolId to its declaring Trait's SymbolId
    pub assoc_type_traits: HashMap<SymbolId, SymbolId>,
    // Maps (ImplKey, assoc_type_sym) to the declared aliased SemanticTypeId in that impl
    pub impl_associated_types: HashMap<(ImplKey, SymbolId), SemanticTypeId>,
    // Maps (trait_sym, name) to assoc_type_sym
    pub assoc_type_names: HashMap<(SymbolId, String), SymbolId>,
    // Maps a generic param symbol to its associated type equality bounds: (trait_id, assoc_type_sym, target_ty)
    pub assoc_type_bounds: HashMap<SymbolId, Vec<(SymbolId, SymbolId, SemanticTypeId)>>,
    // Maps ImplKey to lowered SemanticTypeId of self_type in that impl (e.g., Result<T, E>)
    pub impl_self_types: HashMap<ImplKey, SemanticTypeId>,
    // Maps ImplKey to list of generic parameter symbols declared on the impl block
    pub impl_generic_params: HashMap<ImplKey, Vec<SymbolId>>,
    pub trait_impl_entries: Vec<TraitImplEntry>,
    pub decl_associated_types: HashMap<(DeclId, SymbolId), SemanticTypeId>,

    pub macro_decls: HashMap<SymbolId, DeclId>,
    pub decl_macros: HashMap<DeclId, SymbolId>,
    pub function_effects: HashMap<SymbolId, crate::effect::EffectSet>,
    pub unsafe_functions: HashSet<SymbolId>,
    pub unsafe_function_types: HashSet<SemanticTypeId>,
    pub fn_lifetime_contracts: HashMap<SymbolId, crate::CanonicalLifetimeContract>,
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
            try_branch_methods: HashMap::new(),
            try_from_residual_methods: HashMap::new(),
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
            coercions: HashMap::new(),
            try_branches: HashMap::new(),
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
            trait_associated_types: HashMap::new(),
            assoc_type_traits: HashMap::new(),
            impl_associated_types: HashMap::new(),
            assoc_type_names: HashMap::new(),
            assoc_type_bounds: HashMap::new(),
            impl_self_types: HashMap::new(),
            impl_generic_params: HashMap::new(),
            trait_impl_entries: Vec::new(),
            decl_associated_types: HashMap::new(),
            macro_decls: HashMap::new(),
            decl_macros: HashMap::new(),
            function_effects: HashMap::new(),
            unsafe_functions: HashSet::new(),
            unsafe_function_types: HashSet::new(),
            fn_lifetime_contracts: HashMap::new(),
        }
    }
}
