use crate::registry::{CanonicalSymbolId, ExternalImplKey, ExternalSymbol, ExternalTraitImplEntry, ModuleRegistry, ProviderInterface};
use luna_llib::metadata::{
    CanonicalInterface, CanonicalLifetime, CanonicalType, ExportedSymbol, ImplHeader,
    SemanticMetadata, StableSymbolId, TraitDefinition,
};
use luna_semantic::ty::{SemanticType, SemanticTypeId, TypeContext};
use std::collections::HashMap;

pub struct MetadataBuilder<'a> {
    registry: &'a ModuleRegistry,
    provider: &'a ProviderInterface,
    type_map: HashMap<SemanticTypeId, u32>,
    canonical_types: Vec<CanonicalType>,
}

impl<'a> MetadataBuilder<'a> {
    pub fn new(registry: &'a ModuleRegistry, provider: &'a ProviderInterface) -> Self {
        Self {
            registry,
            provider,
            type_map: HashMap::new(),
            canonical_types: Vec::new(),
        }
    }

    pub fn build(mut self) -> SemanticMetadata {
        // Pre-allocate to prevent infinite recursion on self-referential types
        // Actually we should traverse and build

        let mut exported_symbols = HashMap::new();
        for (name, ext_sym) in &self.provider.exported_symbols {
            let exported = self.convert_exported_symbol(ext_sym);
            exported_symbols.insert(name.clone(), exported);
        }

        let mut traits = HashMap::new();
        // The trait implementations and definitions
        // Wait, how do we get TraitDefinition?
        // luna_semantic hasn't exposed trait fully, but we have trait_impl_entries
        // We will build what we can

        let mut impl_headers = Vec::new();
        let mut processed_keys = std::collections::HashSet::new();

        for (impl_key, method_canons) in &self.provider.impl_methods {
            processed_keys.insert(impl_key.clone());
            let trait_id = impl_key.trait_id.as_ref().map(|t| self.convert_symbol_id(t));
            let self_type = if let Some(&st) = self.provider.impl_self_types.get(impl_key) {
                self.convert_type_id(st)
            } else {
                let canon_def = &impl_key.self_type_def;
                let stable = self.convert_symbol_id(canon_def);
                let ty = self.canonical_types.len() as u32;
                self.canonical_types.push(CanonicalType::Struct(stable, vec![], vec![]));
                ty
            };

            let generic_params = self.provider.impl_generic_params.get(impl_key)
                .map(|gps| gps.iter().map(|p| self.convert_symbol_id(p)).collect())
                .unwrap_or_default();

            let mut methods = HashMap::new();
            for m_canon in method_canons {
                let m_ty_opt = self.provider.impl_method_symbols.iter()
                    .find(|s| s.sym.name == m_canon.name)
                    .and_then(|s| self.provider.symbol_types.get(&s.sym.id).copied());
                if let Some(m_ty) = m_ty_opt {
                    let ty_idx = self.convert_type_id(m_ty);
                    methods.insert(m_canon.name.clone(), ty_idx);
                }
            }

            impl_headers.push(ImplHeader {
                trait_id,
                self_type,
                generic_params,
                methods,
            });
        }

        for entry in &self.provider.trait_impl_entries {
            let key = ExternalImplKey {
                trait_id: Some(entry.trait_id.clone()),
                self_type_def: self.get_canonical_from_type(entry.self_type).unwrap_or_else(|| entry.trait_id.clone()),
            };
            if !processed_keys.contains(&key) {
                let trait_id = Some(self.convert_symbol_id(&entry.trait_id));
                let self_type = self.convert_type_id(entry.self_type);
                let generic_params = entry.generic_params.iter().map(|p| self.convert_symbol_id(p)).collect();
                impl_headers.push(ImplHeader {
                    trait_id,
                    self_type,
                    generic_params,
                    methods: HashMap::new(),
                });
            }
        }

        let interface = CanonicalInterface {
            exported_symbols,
            types: self.canonical_types.clone(),
            traits,
            impl_headers,
        };

        SemanticMetadata {
            metadata_version: 1,
            language_version: 1,
            target_triple: "unknown".to_string(), // TODO: extract from options
            interface_hash: [0; 32],              // Set by writer later
            interface,
        }
    }

    fn get_canonical_from_type(&self, ty_id: SemanticTypeId) -> Option<CanonicalSymbolId> {
        let ty = self.provider.types.get(ty_id);
        match ty {
            SemanticType::Struct(sym_id, _, _) | SemanticType::Enum(sym_id, _, _) => {
                self.provider.symbol_canonicals.get(sym_id).cloned()
            }
            _ => None,
        }
    }

    fn convert_symbol_id(&self, sym_id: &CanonicalSymbolId) -> StableSymbolId {
        let provider_name = if sym_id.provider_id.0 == 0 || sym_id.provider_id == self.provider.id {
            self.provider.name.clone()
        } else {
            self.registry.get_provider_name(sym_id.provider_id)
        };
        StableSymbolId {
            provider_name,
            symbol_path: sym_id.name.clone(),
        }
    }

    fn convert_exported_symbol(&mut self, sym: &ExternalSymbol) -> ExportedSymbol {
        // Find its type if it exists: check sym.sym.id first, then merged_ids
        let mut ty_index = None;
        let sid = sym.sym.id;
        if let Some(ty_id) = self.provider.symbol_types.get(&sid) {
            ty_index = Some(self.convert_type_id(*ty_id));
            if sym.sym.name == "Ok" || sym.sym.name == "Result" {
            }
        } else if let Some((_, msid)) = sym.merged_ids.first() {
            if let Some(ty_id) = self.provider.symbol_types.get(msid) {
                ty_index = Some(self.convert_type_id(*ty_id));
                if sym.sym.name == "Ok" || sym.sym.name == "Result" {
                }
            }
        }
        if ty_index.is_none() && (sym.sym.name == "Ok" || sym.sym.name == "Result") {
        }
        
        let mut generic_params = Vec::new();
        let canon_opt = self.provider.symbol_canonicals.get(&sid)
            .or_else(|| sym.merged_ids.first().and_then(|(_, msid)| self.provider.symbol_canonicals.get(msid)));
        if let Some(canon) = canon_opt {
            if let Some(gps) = self.provider.generic_param_symbols.get(canon) {
                generic_params = gps.iter().map(|p| self.convert_symbol_id(p)).collect();
            }
        }

        let kind_str = match sym.sym.kind {
            luna_semantic::symbol::SymbolKind::Function => "Function",
            luna_semantic::symbol::SymbolKind::ExternFunction => "ExternFunction",
            luna_semantic::symbol::SymbolKind::Variable => "Variable",
            luna_semantic::symbol::SymbolKind::Constant => "Constant",
            luna_semantic::symbol::SymbolKind::Struct => "Struct",
            luna_semantic::symbol::SymbolKind::Enum => "Enum",
            luna_semantic::symbol::SymbolKind::EnumVariant(_) => "EnumVariant",
            luna_semantic::symbol::SymbolKind::Trait => "Trait",
            luna_semantic::symbol::SymbolKind::TraitMethod => "TraitMethod",
            luna_semantic::symbol::SymbolKind::Alias => "Alias",
            luna_semantic::symbol::SymbolKind::AssociatedType => "AssociatedType",
            luna_semantic::symbol::SymbolKind::Module => "Module",
            luna_semantic::symbol::SymbolKind::Type => "Type",
            luna_semantic::symbol::SymbolKind::TypeParam => "TypeParam",
            luna_semantic::symbol::SymbolKind::LifetimeParam => "LifetimeParam",
            luna_semantic::symbol::SymbolKind::Macro => "Macro",
            luna_semantic::symbol::SymbolKind::Unknown => "Unknown",
        }.to_string();

        let mut children = HashMap::new();
        for (c_name, c_sym) in &sym.children {
            children.insert(c_name.clone(), self.convert_exported_symbol(c_sym));
        }

        let lifetime_contract = self.provider.symbol_lifetime_contracts.get(&sid)
            .or_else(|| sym.merged_ids.first().and_then(|(_, msid)| self.provider.symbol_lifetime_contracts.get(msid)))
            .cloned();

        ExportedSymbol {
            kind: kind_str,
            ty_index,
            visibility: match sym.sym.visibility {
                luna_ast::Visibility::Public => 1,
                luna_ast::Visibility::Internal => 2,
                luna_ast::Visibility::Private => 0,
            },
            generic_params,
            symbol_id: StableSymbolId {
                provider_name: self.provider.name.clone(),
                symbol_path: sym.sym.name.clone(),
            },
            children,
            lifetime_contract,
        }
    }

    fn convert_type_id(&mut self, ty_id: SemanticTypeId) -> u32 {
        if let Some(&idx) = self.type_map.get(&ty_id) {
            return idx;
        }

        let idx = self.canonical_types.len() as u32;
        self.type_map.insert(ty_id, idx);
        
        // Insert a dummy to allocate slot
        self.canonical_types.push(CanonicalType::Error);
        
        let sem_ty = self.provider.types.get(ty_id).clone();
        let canon = self.convert_semantic_type(&sem_ty);
        
        self.canonical_types[idx as usize] = canon;
        idx
    }

    fn convert_semantic_type(&mut self, ty: &SemanticType) -> CanonicalType {
        match ty {
            SemanticType::Primitive(b) => CanonicalType::Primitive(*b),
            SemanticType::Struct(sym_id, targs, _bounds) => {
                let canon_sym = self.provider.symbol_canonicals.get(sym_id).unwrap();
                let stable_id = self.convert_symbol_id(canon_sym);
                let args = targs.iter().map(|t| self.convert_type_id(*t)).collect();
                CanonicalType::Struct(stable_id, args, vec![])
            }
            SemanticType::Enum(sym_id, targs, _bounds) => {
                let canon_sym = self.provider.symbol_canonicals.get(sym_id).unwrap();
                let stable_id = self.convert_symbol_id(canon_sym);
                let args = targs.iter().map(|t| self.convert_type_id(*t)).collect();
                CanonicalType::Enum(stable_id, args, vec![])
            }
            SemanticType::Tuple(tys) => {
                let args = tys.iter().map(|t| self.convert_type_id(*t)).collect();
                CanonicalType::Tuple(args)
            }
            SemanticType::Array(t, len) => CanonicalType::Array(self.convert_type_id(*t), *len),
            SemanticType::Slice(t) => CanonicalType::Slice(self.convert_type_id(*t)),
            SemanticType::Function { params, return_type } => CanonicalType::Function {
                params: params.iter().map(|t| self.convert_type_id(*t)).collect(),
                return_type: self.convert_type_id(*return_type),
            },
            SemanticType::Pointer(mutability, t) => CanonicalType::Pointer(mutability.clone(), self.convert_type_id(*t)),
            SemanticType::Reference(_lt, mutability, t) => CanonicalType::Reference(CanonicalLifetime::Anonymous, mutability.clone(), self.convert_type_id(*t)),
            SemanticType::Void => CanonicalType::Void,
            SemanticType::Never => CanonicalType::Never,
            SemanticType::Error | SemanticType::InferenceVar(_) => CanonicalType::Error,
            SemanticType::GenericParam(sym_id) => {
                let canon_sym = self.provider.symbol_canonicals.get(sym_id).unwrap();
                CanonicalType::GenericParam(self.convert_symbol_id(canon_sym))
            }
            SemanticType::Box(t) => CanonicalType::Box(self.convert_type_id(*t)),
            SemanticType::Closure(expr_id, captures, ret) => {
                let caps = captures.iter().map(|t| self.convert_type_id(*t)).collect();
                CanonicalType::Closure(expr_id.0 as u64, caps, self.convert_type_id(*ret))
            }
            SemanticType::DynTrait(sym_id) => {
                let canon_sym = self.provider.symbol_canonicals.get(sym_id).unwrap();
                CanonicalType::DynTrait(self.convert_symbol_id(canon_sym))
            }
            SemanticType::Future(t) => CanonicalType::Future(self.convert_type_id(*t)),
            SemanticType::Range(t) => CanonicalType::Range(self.convert_type_id(*t)),
            SemanticType::Projection { self_type, trait_id, assoc_type } => {
                let canon_trait = self.provider.symbol_canonicals.get(trait_id).unwrap();
                let canon_assoc = self.provider.symbol_canonicals.get(assoc_type).unwrap();
                CanonicalType::Projection {
                    self_type: self.convert_type_id(*self_type),
                    trait_id: self.convert_symbol_id(canon_trait),
                    assoc_type: self.convert_symbol_id(canon_assoc),
                }
            }
        }
    }
}