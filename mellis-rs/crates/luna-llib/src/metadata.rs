use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use luna_semantic::ty::{BuiltinType, Mutability};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMetadata {
    pub metadata_version: u16,        // Always 1 for v1
    pub language_version: u16,        // Core language version compatibility
    pub target_triple: String,        // Target architecture
    pub interface_hash: [u8; 32],     // Cryptographic fingerprint
    pub interface: CanonicalInterface,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StableSymbolId {
    pub provider_name: String,
    pub symbol_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CanonicalLifetime {
    Static,
    Named(String),
    Anonymous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CanonicalType {
    Primitive(BuiltinType),
    Struct(StableSymbolId, Vec<u32>, Vec<u32>), // trait bounds and generic arguments as type indices
    Enum(StableSymbolId, Vec<u32>, Vec<u32>),
    Tuple(Vec<u32>),
    Array(u32, u64),
    Slice(u32),
    Function { params: Vec<u32>, return_type: u32 },
    Pointer(Mutability, u32),
    Reference(CanonicalLifetime, Mutability, u32),
    Void,
    Never,
    Error,
    GenericParam(StableSymbolId),
    Box(u32),
    Closure(u64, Vec<u32>, u32), // unique id, captures, return type
    DynTrait(StableSymbolId),
    Future(u32),
    Range(u32),
    Projection { self_type: u32, trait_id: StableSymbolId, assoc_type: StableSymbolId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalInterface {
    pub exported_symbols: HashMap<String, ExportedSymbol>,
    pub types: Vec<CanonicalType>, // Deduplicated type registry for this ABI
    pub traits: HashMap<StableSymbolId, TraitDefinition>,
    pub impl_headers: Vec<ImplHeader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedSymbol {
    pub kind: String, // "Function", "Struct", etc.
    pub ty_index: Option<u32>, // Index into CanonicalInterface::types, None for modules
    pub visibility: u8,
    pub generic_params: Vec<StableSymbolId>,
    pub symbol_id: StableSymbolId,
    #[serde(default)]
    pub children: HashMap<String, ExportedSymbol>,
    #[serde(default)]
    pub lifetime_contract: Option<luna_semantic::CanonicalLifetimeContract>,
    #[serde(default)]
    pub is_unsafe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitDefinition {
    pub name: String,
    pub methods: HashMap<String, u32>, // Method name -> Type index
    pub associated_types: Vec<StableSymbolId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplHeader {
    pub trait_id: Option<StableSymbolId>, // None for inherent impls
    pub self_type: u32,
    #[serde(default)]
    pub generic_params: Vec<StableSymbolId>,
    pub methods: HashMap<String, u32>, // Method name -> Type index
}