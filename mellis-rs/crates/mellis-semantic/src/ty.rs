use mellis_common::ids::SymbolId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticTypeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BuiltinType {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
    Bool,
    String,
    Char,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mutability {
    Mutable,
    Immutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LifetimeId(pub u32);

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Substitution {
    pub map: std::collections::BTreeMap<SymbolId, SemanticTypeId>,
}

impl Substitution {
    pub fn new() -> Self {
        Self { map: std::collections::BTreeMap::new() }
    }
    
    pub fn insert(&mut self, param: SymbolId, ty: SemanticTypeId) {
        self.map.insert(param, ty);
    }
    
    pub fn get(&self, param: SymbolId) -> Option<&SemanticTypeId> {
        self.map.get(&param)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SemanticType {
    Primitive(BuiltinType),
    Struct(SymbolId, Vec<SemanticTypeId>, Vec<SemanticTypeId>),
    Enum(SymbolId, Vec<SemanticTypeId>, Vec<SemanticTypeId>),
    Tuple(Vec<SemanticTypeId>),
    Array(SemanticTypeId, u64),
    Slice(SemanticTypeId),
    Function { params: Vec<SemanticTypeId>, return_type: SemanticTypeId },
    Pointer(Mutability, SemanticTypeId),
    Reference(LifetimeId, Mutability, SemanticTypeId),
    Void,
    Never,
    Error,
    InferenceVar(u32),
    GenericParam(SymbolId),
    Box(SemanticTypeId),
    Closure(mellis_ast::ExprId, Vec<SemanticTypeId>, SemanticTypeId),
    DynTrait(SymbolId),
    Future(SemanticTypeId),
    Range(SemanticTypeId),
}

#[derive(Clone, Debug)]
pub struct TypeContext {
    types: Vec<SemanticType>,
    type_interner: HashMap<SemanticType, SemanticTypeId>,
    next_inference_var: u32,
    pub inference_bindings: std::collections::BTreeMap<u32, SemanticTypeId>,
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            types: Vec::new(),
            type_interner: HashMap::new(),
            next_inference_var: 0,
            inference_bindings: std::collections::BTreeMap::new(),
        };
        // Pre-populate standard types so they have fixed IDs if we want
        ctx.intern(SemanticType::Void);
        ctx.intern(SemanticType::Error);
        ctx.intern(SemanticType::Never);
        ctx.intern(SemanticType::Primitive(BuiltinType::I32));
        ctx.intern(SemanticType::Primitive(BuiltinType::Bool));
        ctx.intern(SemanticType::Primitive(BuiltinType::F64));
        ctx.intern(SemanticType::Primitive(BuiltinType::String));
        ctx
    }

    pub fn intern(&mut self, ty: SemanticType) -> SemanticTypeId {
        if let Some(&id) = self.type_interner.get(&ty) {
            return id;
        }
        let id = SemanticTypeId(self.types.len() as u32);
        self.type_interner.insert(ty.clone(), id);
        self.types.push(ty);
        id
    }

    pub fn bool_id(&self) -> SemanticTypeId {
        self.type_interner
            .get(&SemanticType::Primitive(BuiltinType::Bool))
            .copied()
            .expect("Bool primitive must be pre-populated")
    }

    pub fn get(&self, id: SemanticTypeId) -> &SemanticType {
        &self.types[id.0 as usize]
    }

    pub fn new_inference_var(&mut self) -> SemanticTypeId {
        let id = self.next_inference_var;
        self.next_inference_var += 1;
        self.intern(SemanticType::InferenceVar(id))
    }
    
    pub fn resolve_inference(&self, id: SemanticTypeId) -> SemanticTypeId {
        let mut current = id;
        let mut seen = std::collections::HashSet::new();
        loop {
            let ty = self.get(current);
            let SemanticType::InferenceVar(var_id) = ty else { return current };
            if !seen.insert(*var_id) { return current; }
            let Some(&bound) = self.inference_bindings.get(var_id) else { return current };
            current = bound;
        }
    }
    
    pub fn subst(&mut self, id: SemanticTypeId, subst: &Substitution) -> SemanticTypeId {
        let id = self.resolve(id);
        let ty = self.get(id).clone();
        match ty {
            SemanticType::GenericParam(sym_id) => {
                if let Some(&new_id) = subst.get(sym_id) {
                    if new_id == id { id } else { self.subst(new_id, subst) }
                } else {
                    id
                }
            }
            SemanticType::Struct(sym, args, fields) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.subst(a, subst)).collect();
                let new_fields: Vec<_> = fields.iter().map(|&f| self.subst(f, subst)).collect();
                self.intern(SemanticType::Struct(sym, new_args, new_fields))
            }
            SemanticType::Enum(sym, args, variants) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.subst(a, subst)).collect();
                let new_variants: Vec<_> = variants.iter().map(|&v| self.subst(v, subst)).collect();
                self.intern(SemanticType::Enum(sym, new_args, new_variants))
            }
            SemanticType::Tuple(args) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.subst(a, subst)).collect();
                self.intern(SemanticType::Tuple(new_args))
            }
            SemanticType::Function { params, return_type } => {
                let new_params: Vec<_> = params.iter().map(|&p| self.subst(p, subst)).collect();
                let new_ret = self.subst(return_type, subst);
                self.intern(SemanticType::Function { params: new_params, return_type: new_ret })
            }
            SemanticType::Pointer(mutability, inner) => {
                let new_inner = self.subst(inner, subst);
                self.intern(SemanticType::Pointer(mutability, new_inner))
            }
            SemanticType::Reference(lt, mutability, inner) => {
                let new_inner = self.subst(inner, subst);
                self.intern(SemanticType::Reference(lt, mutability, new_inner))
            }
            SemanticType::Box(inner) => {
                let new_inner = self.subst(inner, subst);
                self.intern(SemanticType::Box(new_inner))
            }
            SemanticType::Closure(expr_id, params, return_type) => {
                let new_params = params.iter().map(|&param| self.subst(param, subst)).collect();
                let new_return_type = self.subst(return_type, subst);
                self.intern(SemanticType::Closure(expr_id, new_params, new_return_type))
            }
            SemanticType::Future(inner) => {
                let new_inner = self.subst(inner, subst);
                self.intern(SemanticType::Future(new_inner))
            }
            _ => id, // Primitive, Void, Error, Never, InferenceVar
        }
    }
    
    pub fn resolve(&self, id: SemanticTypeId) -> SemanticTypeId {
        let mut current = id;
        loop {
            let ty = self.get(current).clone();
            if let SemanticType::InferenceVar(var_id) = ty {
                if let Some(&bound) = self.inference_bindings.get(&var_id) {
                    current = bound;
                    continue;
                }
            }
            break;
        }
        current
    }

    pub fn clone_type_from(&mut self, id: SemanticTypeId, source_ctx: &TypeContext, symbol_map: &std::collections::HashMap<SymbolId, SymbolId>) -> SemanticTypeId {
        let ty = source_ctx.get(id).clone();
        match ty {
            SemanticType::Primitive(p) => self.intern(SemanticType::Primitive(p)),

            SemanticType::Struct(sym, args, fields) => {
                let new_sym = *symbol_map.get(&sym).unwrap_or(&sym);
                let new_args = args.iter().map(|&a| self.clone_type_from(a, source_ctx, symbol_map)).collect();
                let new_fields = fields.iter().map(|&f| self.clone_type_from(f, source_ctx, symbol_map)).collect();
                self.intern(SemanticType::Struct(new_sym, new_args, new_fields))
            }
            SemanticType::Enum(sym, args, variants) => {
                let new_sym = *symbol_map.get(&sym).unwrap_or(&sym);
                let new_args = args.iter().map(|&a| self.clone_type_from(a, source_ctx, symbol_map)).collect();
                let new_variants = variants.iter().map(|&v| self.clone_type_from(v, source_ctx, symbol_map)).collect();
                self.intern(SemanticType::Enum(new_sym, new_args, new_variants))
            }
            SemanticType::Tuple(args) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.clone_type_from(a, source_ctx, symbol_map)).collect();
                self.intern(SemanticType::Tuple(new_args))
            }
            SemanticType::Array(elem, len) => {
                let new_elem = self.clone_type_from(elem, source_ctx, symbol_map);
                self.intern(SemanticType::Array(new_elem, len))
            }
            SemanticType::Slice(elem) => {
                let new_elem = self.clone_type_from(elem, source_ctx, symbol_map);
                self.intern(SemanticType::Slice(new_elem))
            }
            SemanticType::Function { params, return_type } => {
                let new_params: Vec<_> = params.iter().map(|&p| self.clone_type_from(p, source_ctx, symbol_map)).collect();
                let new_ret = self.clone_type_from(return_type, source_ctx, symbol_map);
                self.intern(SemanticType::Function { params: new_params, return_type: new_ret })
            }
            SemanticType::Pointer(mutability, inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, symbol_map);
                self.intern(SemanticType::Pointer(mutability, new_inner))
            }
            SemanticType::Reference(lt, mutability, inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, symbol_map);
                self.intern(SemanticType::Reference(lt, mutability, new_inner))
            }
            SemanticType::Box(inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, symbol_map);
                self.intern(SemanticType::Box(new_inner))
            }
            SemanticType::GenericParam(sym) => {
                let new_sym = *symbol_map.get(&sym).unwrap_or(&sym);
                self.intern(SemanticType::GenericParam(new_sym))
            }
            SemanticType::InferenceVar(v) => {
                if let Some(&bound) = source_ctx.inference_bindings.get(&v) {
                    self.clone_type_from(bound, source_ctx, symbol_map)
                } else {
                    self.new_inference_var()
                }
            }
            SemanticType::Void => self.intern(SemanticType::Void),
            SemanticType::Never => self.intern(SemanticType::Never),
            SemanticType::Error => self.intern(SemanticType::Error),
            SemanticType::Closure(expr_id, params, ret) => {
                let new_params = params.iter().map(|p| self.clone_type_from(*p, source_ctx, symbol_map)).collect();
                let new_ret = self.clone_type_from(ret, source_ctx, symbol_map);
                self.intern(SemanticType::Closure(expr_id, new_params, new_ret))
            }
            SemanticType::DynTrait(sym) => {
                let new_sym = *symbol_map.get(&sym).unwrap_or(&sym);
                self.intern(SemanticType::DynTrait(new_sym))
            }
            SemanticType::Future(inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, symbol_map);
                self.intern(SemanticType::Future(new_inner))
            }
            SemanticType::Range(inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, symbol_map);
                self.intern(SemanticType::Range(new_inner))
            }
        }
    }
}
