use crate::registry::{CanonicalSymbolId, ExternalImplKey, ExternalSymbol, ExternalTraitImplEntry, ProviderInterface};
use luna_llib::metadata::{CanonicalInterface, CanonicalType, CanonicalLifetime, SemanticMetadata, StableSymbolId};
use luna_semantic::ty::{BuiltinType, Mutability, SemanticType, SemanticTypeId, TypeContext};
use luna_semantic::symbol::ProviderId;
use std::collections::HashMap;

pub struct InterfaceDecoder {
    provider_id: ProviderId,
    provider_name: String,
    interface: CanonicalInterface,
    symbol_id_allocator: u32,
    stable_to_local_sym: HashMap<StableSymbolId, luna_common::ids::SymbolId>,
    canonical_syms: HashMap<luna_common::ids::SymbolId, CanonicalSymbolId>,
    type_map: HashMap<u32, SemanticTypeId>,
    types: TypeContext,
    impl_method_symbols: Vec<ExternalSymbol>,
    impl_generic_param_symbols: Vec<ExternalSymbol>,
    allocated_generic_params: std::collections::HashSet<luna_common::ids::SymbolId>,
    symbol_lifetime_contracts: HashMap<luna_common::ids::SymbolId, luna_semantic::CanonicalLifetimeContract>,
    known_providers: HashMap<String, ProviderId>,
}

impl InterfaceDecoder {
    pub fn new(
        provider_id: ProviderId,
        provider_name: String,
        metadata: SemanticMetadata,
        known_providers: HashMap<String, ProviderId>,
    ) -> Self {
        Self {
            provider_id,
            provider_name,
            interface: metadata.interface,
            symbol_id_allocator: 1, // Start from 1
            stable_to_local_sym: HashMap::new(),
            canonical_syms: HashMap::new(),
            type_map: HashMap::new(),
            types: TypeContext::new(),
            impl_method_symbols: Vec::new(),
            impl_generic_param_symbols: Vec::new(),
            allocated_generic_params: std::collections::HashSet::new(),
            symbol_lifetime_contracts: HashMap::new(),
            known_providers,
        }
    }

    fn ensure_generic_param_symbol(&mut self, sym_id: luna_common::ids::SymbolId, name: &str) {
        if self.allocated_generic_params.insert(sym_id) {
            let ext_sym = ExternalSymbol::new(luna_semantic::symbol::Symbol {
                id: sym_id,
                name: name.to_string(),
                ctxt: luna_common::ids::SyntaxContext(0),
                kind: luna_semantic::symbol::SymbolKind::TypeParam,
                scope: luna_semantic::symbol::ScopeId(0),
                span: luna_common::Span::new(luna_common::ids::FileId(0), 0, 0),
                visibility: luna_ast::Visibility::Public,
                decl_id: Some(luna_ast::DeclId(sym_id.0)),
                inner_scope: None,
                provider_id: Some(self.provider_id),
            });
            self.impl_generic_param_symbols.push(ext_sym);
        }
    }

    pub fn decode(mut self) -> ProviderInterface {
        // Decode all types to populate type context first
        for i in 0..self.interface.types.len() {
            self.decode_type_index(i as u32);
        }

        let mut exported_symbols = HashMap::new();
        let mut symbol_types = HashMap::new();
        let mut generic_param_symbols = HashMap::new();

        for (name, exported) in self.interface.exported_symbols.clone() {
            let ext_sym = self.decode_symbol(&name, &exported);
            self.collect_symbol_data(&exported, &mut symbol_types, &mut generic_param_symbols);
            exported_symbols.insert(name, ext_sym);
        }

        let mut trait_methods = HashMap::new();
        for ext_sym in exported_symbols.values() {
            Self::collect_trait_methods(ext_sym, &self.canonical_syms, &mut trait_methods);
        }

        let mut trait_impl_entries = Vec::new();
        let mut impl_methods = HashMap::new();
        let mut impl_generic_params = HashMap::new();
        let mut impl_self_types = HashMap::new();

        for impl_header in self.interface.impl_headers.clone() {
            let trait_id = impl_header.trait_id.as_ref().map(|t| {
                let local_t = self.allocate_sym(t);
                self.canonical_syms[&local_t].clone()
            });
            let self_type = self.type_map[&impl_header.self_type];

            // Resolve self_type_def
            let sem_self_ty = self.types.get(self_type).clone();
            let self_type_def = match &sem_self_ty {
                SemanticType::Struct(sym_id, _, _) | SemanticType::Enum(sym_id, _, _) => {
                    self.canonical_syms[sym_id].clone()
                }
                _ => {
                    trait_id.clone().unwrap_or_else(|| CanonicalSymbolId {
                        provider_id: self.provider_id,
                        decl_id: None,
                        name: "unknown".to_string(),
                    })
                }
            };

            let key = ExternalImplKey {
                trait_id: trait_id.clone(),
                self_type_def,
            };

            let mut canon_gps = Vec::new();
            for gp in &impl_header.generic_params {
                let local_gp = self.allocate_sym(gp);
                self.ensure_generic_param_symbol(local_gp, &gp.symbol_path);
                canon_gps.push(self.canonical_syms[&local_gp].clone());
            }

            if let Some(t_id) = trait_id {
                trait_impl_entries.push(ExternalTraitImplEntry {
                    decl_id: None,
                    trait_id: t_id,
                    self_type,
                    generic_params: canon_gps.clone(),
                });
            }

            let mut method_canons = Vec::new();
            for (m_name, m_ty_idx) in impl_header.methods {
                let m_stable = StableSymbolId {
                    provider_name: self.provider_name.clone(),
                    symbol_path: m_name.clone(),
                };
                let m_sym = self.allocate_sym(&m_stable);
                let m_canon = self.canonical_syms[&m_sym].clone();
                if let Some(&m_sem_ty) = self.type_map.get(&m_ty_idx) {
                    symbol_types.insert(m_sym, m_sem_ty);
                }
                let ext_sym = ExternalSymbol::new(luna_semantic::symbol::Symbol {
                    id: m_sym,
                    name: m_name,
                    ctxt: luna_common::ids::SyntaxContext(0),
                    kind: luna_semantic::symbol::SymbolKind::Function,
                    scope: luna_semantic::symbol::ScopeId(0),
                    span: luna_common::Span::new(luna_common::ids::FileId(0), 0, 0),
                    visibility: luna_ast::Visibility::Public,
                    decl_id: Some(luna_ast::DeclId(m_sym.0)),
                    inner_scope: None,
                    provider_id: Some(self.provider_id),
                });
                self.impl_method_symbols.push(ext_sym);
                method_canons.push(m_canon);
            }
            impl_methods.entry(key.clone()).or_insert_with(Vec::new).extend(method_canons);
            if !canon_gps.is_empty() {
                impl_generic_params.insert(key.clone(), canon_gps);
            }
            impl_self_types.insert(key, self_type);
        }

        ProviderInterface {
            id: self.provider_id,
            name: self.provider_name,
            exported_symbols,
            symbol_types,
            types: self.types,
            lang_items: HashMap::new(), // Not implemented in v1 SemanticMetadata
            generic_param_symbols,
            trait_impls: HashMap::new(),
            trait_impl_entries,
            impl_methods,
            method_impls: HashMap::new(),
            impl_method_symbols: self.impl_method_symbols,
            symbol_canonicals: self.canonical_syms,
            trait_associated_types: HashMap::new(),
            impl_associated_types: HashMap::new(),
            impl_self_types,
            impl_generic_params,
            impl_generic_param_symbols: self.impl_generic_param_symbols,
            trait_associated_type_symbols: Vec::new(),
            internal_symbols: Vec::new(),
            decl_symbols: HashMap::new(),
            expr_symbols: HashMap::new(),
            pat_symbols: HashMap::new(),
            pat_types: HashMap::new(),
            expr_types: HashMap::new(),
            expr_substs: HashMap::new(),
            expr_struct_init_indices: HashMap::new(),
            raw_generic_param_symbols: HashMap::new(),
            symbol_lifetime_contracts: self.symbol_lifetime_contracts,
            trait_methods,
        }
    }

    fn collect_trait_methods(
        ext_sym: &ExternalSymbol,
        canonical_syms: &HashMap<luna_common::ids::SymbolId, CanonicalSymbolId>,
        trait_methods: &mut HashMap<CanonicalSymbolId, Vec<CanonicalSymbolId>>,
    ) {
        if ext_sym.sym.kind == luna_semantic::symbol::SymbolKind::Trait {
            if let Some(canon_trait) = canonical_syms.get(&ext_sym.sym.id) {
                let mut meths: Vec<&ExternalSymbol> = ext_sym.children.values().filter(|c| {
                    c.sym.kind == luna_semantic::symbol::SymbolKind::TraitMethod
                        || c.sym.kind == luna_semantic::symbol::SymbolKind::Function
                }).collect();
                meths.sort_by_key(|m| m.sym.id.0);
                let canon_meths: Vec<CanonicalSymbolId> = meths.iter()
                    .filter_map(|m| canonical_syms.get(&m.sym.id).cloned())
                    .collect();
                trait_methods.insert(canon_trait.clone(), canon_meths);
            }
        }
        for child in ext_sym.children.values() {
            Self::collect_trait_methods(child, canonical_syms, trait_methods);
        }
    }

    fn decode_symbol(&mut self, name: &str, exported: &luna_llib::metadata::ExportedSymbol) -> ExternalSymbol {
        let sym_id = self.allocate_sym(&exported.symbol_id);
        let kind = match exported.kind.as_str() {
            "Function" => luna_semantic::symbol::SymbolKind::Function,
            "ExternFunction" => luna_semantic::symbol::SymbolKind::ExternFunction,
            "Variable" => luna_semantic::symbol::SymbolKind::Variable,
            "Constant" => luna_semantic::symbol::SymbolKind::Constant,
            "Struct" => luna_semantic::symbol::SymbolKind::Struct,
            "Enum" => luna_semantic::symbol::SymbolKind::Enum,
            "EnumVariant" => luna_semantic::symbol::SymbolKind::EnumVariant(0),
            "Trait" => luna_semantic::symbol::SymbolKind::Trait,
            "TraitMethod" => luna_semantic::symbol::SymbolKind::TraitMethod,
            "Alias" => luna_semantic::symbol::SymbolKind::Alias,
            "AssociatedType" => luna_semantic::symbol::SymbolKind::AssociatedType,
            "Module" => luna_semantic::symbol::SymbolKind::Module,
            "Type" => luna_semantic::symbol::SymbolKind::Type,
            "TypeParam" => luna_semantic::symbol::SymbolKind::TypeParam,
            "LifetimeParam" => luna_semantic::symbol::SymbolKind::LifetimeParam,
            "Macro" => luna_semantic::symbol::SymbolKind::Macro,
            _ => luna_semantic::symbol::SymbolKind::Unknown,
        };
        let visibility = match exported.visibility {
            1 => luna_ast::Visibility::Public,
            2 => luna_ast::Visibility::Internal,
            _ => luna_ast::Visibility::Private,
        };

        let mut children = HashMap::new();
        for (c_name, c_exported) in &exported.children {
            let child_sym = self.decode_symbol(c_name, c_exported);
            children.insert(c_name.clone(), child_sym);
        }

        let mut ext_sym = ExternalSymbol::new(luna_semantic::symbol::Symbol {
            id: sym_id,
            name: name.to_string(),
            ctxt: luna_common::ids::SyntaxContext(0),
            kind,
            scope: luna_semantic::symbol::ScopeId(0),
            span: luna_common::Span::new(luna_common::ids::FileId(0), 0, 0),
            visibility,
            decl_id: Some(luna_ast::DeclId(sym_id.0)),
            inner_scope: None,
            provider_id: Some(self.provider_id),
        });
        ext_sym.children = children;
        ext_sym.merged_ids.push((self.provider_id, sym_id));
        ext_sym
    }

    fn collect_symbol_data(
        &mut self,
        exported: &luna_llib::metadata::ExportedSymbol,
        symbol_types: &mut HashMap<luna_common::ids::SymbolId, SemanticTypeId>,
        generic_param_symbols: &mut HashMap<CanonicalSymbolId, Vec<CanonicalSymbolId>>,
    ) {
        let sym_id = self.allocate_sym(&exported.symbol_id);
        if let Some(contract) = &exported.lifetime_contract {
            self.symbol_lifetime_contracts.insert(sym_id, contract.clone());
        }
        if let Some(ty_idx) = exported.ty_index {
            if let Some(&sem_ty_id) = self.type_map.get(&ty_idx) {
                symbol_types.insert(sym_id, sem_ty_id);
            }
        }
        if !exported.generic_params.is_empty() {
            let canon = self.canonical_syms[&sym_id].clone();
            let mut params = Vec::new();
            for gp in &exported.generic_params {
                let local_gp = self.allocate_sym(gp);
                self.ensure_generic_param_symbol(local_gp, &gp.symbol_path);
                params.push(self.canonical_syms[&local_gp].clone());
            }
            generic_param_symbols.insert(canon, params);
        }
        for child in exported.children.values() {
            self.collect_symbol_data(child, symbol_types, generic_param_symbols);
        }
    }

    fn allocate_sym(&mut self, stable: &StableSymbolId) -> luna_common::ids::SymbolId {
        if let Some(&id) = self.stable_to_local_sym.get(stable) {
            return id;
        }
        let id = luna_common::ids::SymbolId(self.symbol_id_allocator);
        self.symbol_id_allocator += 1;
        self.stable_to_local_sym.insert(stable.clone(), id);
        
        let pid = if stable.provider_name == self.provider_name {
            self.provider_id
        } else if let Some(&other_pid) = self.known_providers.get(&stable.provider_name) {
            other_pid
        } else {
            self.provider_id
        };
        let decl_id = if pid == self.provider_id {
            Some(luna_ast::DeclId(id.0))
        } else {
            None
        };
        let canon = CanonicalSymbolId {
            provider_id: pid,
            decl_id,
            name: stable.symbol_path.clone(),
        };
        self.canonical_syms.insert(id, canon);
        id
    }

    fn decode_type_index(&mut self, index: u32) -> SemanticTypeId {
        if let Some(&id) = self.type_map.get(&index) {
            return id;
        }
        // To handle recursion safely without infinite loop, insert a temporary Error
        let temp_id = self.types.intern(SemanticType::Error);
        self.type_map.insert(index, temp_id);

        let canon = self.interface.types[index as usize].clone();
        let sem_ty = match canon {
            CanonicalType::Primitive(b) => SemanticType::Primitive(b),
            CanonicalType::Struct(sym, args, _) => {
                let s_id = self.allocate_sym(&sym);
                let s_args = args.into_iter().map(|a| self.decode_type_index(a)).collect();
                SemanticType::Struct(s_id, s_args, vec![])
            }
            CanonicalType::Enum(sym, args, _) => {
                let s_id = self.allocate_sym(&sym);
                let s_args = args.into_iter().map(|a| self.decode_type_index(a)).collect();
                SemanticType::Enum(s_id, s_args, vec![])
            }
            CanonicalType::Tuple(args) => {
                let s_args = args.into_iter().map(|a| self.decode_type_index(a)).collect();
                SemanticType::Tuple(s_args)
            }
            CanonicalType::Array(t, len) => SemanticType::Array(self.decode_type_index(t), len),
            CanonicalType::Slice(t) => SemanticType::Slice(self.decode_type_index(t)),
            CanonicalType::Function { params, return_type } => SemanticType::Function {
                params: params.into_iter().map(|p| self.decode_type_index(p)).collect(),
                return_type: self.decode_type_index(return_type),
            },
            CanonicalType::Pointer(mutability, t) => SemanticType::Pointer(mutability, self.decode_type_index(t)),
            CanonicalType::Reference(_, mutability, t) => SemanticType::Reference(luna_semantic::ty::LifetimeId(0), mutability, self.decode_type_index(t)),
            CanonicalType::Void => SemanticType::Void,
            CanonicalType::Never => SemanticType::Never,
            CanonicalType::Error => SemanticType::Error,
            CanonicalType::GenericParam(sym) => {
                let local = self.allocate_sym(&sym);
                self.ensure_generic_param_symbol(local, &sym.symbol_path);
                SemanticType::GenericParam(local)
            }
            CanonicalType::Box(t) => SemanticType::Box(self.decode_type_index(t)),
            CanonicalType::Closure(expr_id, caps, ret) => {
                let c_caps = caps.into_iter().map(|c| self.decode_type_index(c)).collect();
                SemanticType::Closure(luna_ast::ExprId(expr_id as u32), c_caps, self.decode_type_index(ret))
            }
            CanonicalType::DynTrait(sym) => SemanticType::DynTrait(self.allocate_sym(&sym)),
            CanonicalType::Future(t) => SemanticType::Future(self.decode_type_index(t)),
            CanonicalType::Range(t) => SemanticType::Range(self.decode_type_index(t)),
            CanonicalType::Projection { self_type, trait_id, assoc_type } => SemanticType::Projection {
                self_type: self.decode_type_index(self_type),
                trait_id: self.allocate_sym(&trait_id),
                assoc_type: self.allocate_sym(&assoc_type),
            },
        };

        let real_id = self.types.intern(sem_ty);
        
        // We update type_map. This does NOT replace the Error type in the interner, but 
        // subsequent references to this index will use the real_id.
        // For actual self-referential types, we might have a problem if it resolves to temp_id.
        // But SemanticType Struct/Enum breaks cyclic reference because it doesn't store SemanticTypeId.
        self.type_map.insert(index, real_id);
        real_id
    }
}