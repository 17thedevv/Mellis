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
                let ret = self.intern(SemanticType::Enum(sym, new_args, new_variants));
                if id.0 == 56 || ret.0 == 56 {
                    println!("DEBUG: subst(56) -> Enum({:?}, {:?}, ...) -> ret {}", sym, args, ret.0);
                }
                ret
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
    
    
    pub fn occurs_check(&self, var: u32, ty: SemanticTypeId) -> bool {
        let ty = self.resolve(ty);
        match self.get(ty).clone() {
            SemanticType::InferenceVar(v) => v == var,
            SemanticType::Struct(_, args, fields) => {
                args.iter().any(|&a| self.occurs_check(var, a)) || fields.iter().any(|&f| self.occurs_check(var, f))
            }
            SemanticType::Enum(_, args, variants) => {
                args.iter().any(|&a| self.occurs_check(var, a)) || variants.iter().any(|&v| self.occurs_check(var, v))
            }
            SemanticType::Tuple(args) => args.iter().any(|&a| self.occurs_check(var, a)),
            SemanticType::Array(inner, _) | SemanticType::Slice(inner) | SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) | SemanticType::Box(inner) => {
                self.occurs_check(var, inner)
            }
            SemanticType::Function { params, return_type } => {
                params.iter().any(|&p| self.occurs_check(var, p)) || self.occurs_check(var, return_type)
            }
            SemanticType::Closure(_, captures, ret) => captures.iter().any(|&c| self.occurs_check(var, c)) || self.occurs_check(var, ret),
            SemanticType::Future(inner) => self.occurs_check(var, inner),
            _ => false,
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


    pub fn contains_inference_var(&self, id: SemanticTypeId) -> bool {
        self.type_flags(id).0
    }
    
    pub fn contains_generic_param(&self, id: SemanticTypeId) -> bool {
        self.type_flags(id).1
    }
    
    pub fn is_monomorphic(&self, id: SemanticTypeId) -> bool {
        let flags = self.type_flags(id);
        if flags.1 {
            println!("DEBUG: type_flags for {:?} = {:?}, ty = {:?}", id, flags, self.get(id));
        }
        !flags.0 && !flags.1 && !flags.2 // has_infer, has_generic, has_error
    }
    
    fn type_flags(&self, id: SemanticTypeId) -> (bool, bool, bool) { // (has_infer, has_generic, has_error)
        let resolved = self.resolve_inference(id);
        let ty = self.get(resolved).clone();
        match ty {
            SemanticType::Error => (false, false, true),
            SemanticType::Primitive(_) | SemanticType::Void | SemanticType::Never => (false, false, false),
            SemanticType::InferenceVar(_) => (true, false, false),
            SemanticType::GenericParam(_) => (false, true, false),
            SemanticType::Struct(_, args, fields) | SemanticType::Enum(_, args, fields) => {
                let mut has_infer = false;
                let mut has_gen = false;
                let mut has_err = false;
                for &a in args.iter().chain(fields.iter()) {
                    let (i, g, e) = self.type_flags(a);
                    if i {
                        println!("DEBUG: type_flags inference var at {:?} (resolved: {:?})", a, resolved);
                    }
                    if g {
                        println!("DEBUG: type_flags generic param at {:?} (resolved: {:?}, ty: {:?})", a, resolved, self.get(a));
                    }
                    has_infer |= i; has_gen |= g; has_err |= e;
                    if has_infer && has_gen && has_err { break; }
                }
                (has_infer, has_gen, has_err)
            }
            SemanticType::Tuple(args) => {
                let mut has_infer = false;
                let mut has_gen = false;
                let mut has_err = false;
                for &a in args.iter() {
                    let (i, g, e) = self.type_flags(a);
                    has_infer |= i; has_gen |= g; has_err |= e;
                    if has_infer && has_gen && has_err { break; }
                }
                (has_infer, has_gen, has_err)
            }
            SemanticType::Array(inner, _) | SemanticType::Slice(inner) | SemanticType::Pointer(_, inner) | 
            SemanticType::Reference(_, _, inner) | SemanticType::Box(inner) | SemanticType::Future(inner) | 
            SemanticType::Range(inner) => {
                self.type_flags(inner)
            }
            SemanticType::Function { params, return_type } => {
                let mut has_infer = false;
                let mut has_gen = false;
                let mut has_err = false;
                for &p in params.iter() {
                    let (i, g, e) = self.type_flags(p);
                    has_infer |= i; has_gen |= g; has_err |= e;
                    if has_infer && has_gen && has_err { break; }
                }
                if !has_infer || !has_gen || !has_err {
                    let (i, g, e) = self.type_flags(return_type);
                    has_infer |= i; has_gen |= g; has_err |= e;
                }
                (has_infer, has_gen, has_err)
            }
            SemanticType::Closure(_, params, ret) => {
                let mut has_infer = false;
                let mut has_gen = false;
                let mut has_err = false;
                for &p in params.iter() {
                    let (i, g, e) = self.type_flags(p);
                    has_infer |= i; has_gen |= g; has_err |= e;
                    if has_infer && has_gen && has_err { break; }
                }
                if !has_infer || !has_gen || !has_err {
                    let (i, g, e) = self.type_flags(ret);
                    has_infer |= i; has_gen |= g; has_err |= e;
                }
                (has_infer, has_gen, has_err)
            }
            SemanticType::DynTrait(_) => (false, false, false),
        }
    }

    pub fn clone_type_from<F>(&mut self, id: SemanticTypeId, source_ctx: &TypeContext, lookup_sym: &F) -> SemanticTypeId
    where
        F: Fn(SymbolId) -> SymbolId,
    {
        let ty = source_ctx.get(id).clone();
        match ty {
            SemanticType::Primitive(p) => self.intern(SemanticType::Primitive(p)),

            SemanticType::Struct(sym, args, fields) => {
                let new_sym = lookup_sym(sym);
                let new_args = args.iter().map(|&a| self.clone_type_from(a, source_ctx, lookup_sym)).collect();
                let new_fields = fields.iter().map(|&f| self.clone_type_from(f, source_ctx, lookup_sym)).collect();
                self.intern(SemanticType::Struct(new_sym, new_args, new_fields))
            }
            SemanticType::Enum(sym, args, variants) => {
                let new_sym = lookup_sym(sym);
                let new_args = args.iter().map(|&a| self.clone_type_from(a, source_ctx, lookup_sym)).collect();
                let new_variants = variants.iter().map(|&v| self.clone_type_from(v, source_ctx, lookup_sym)).collect();
                self.intern(SemanticType::Enum(new_sym, new_args, new_variants))
            }
            SemanticType::Tuple(args) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.clone_type_from(a, source_ctx, lookup_sym)).collect();
                self.intern(SemanticType::Tuple(new_args))
            }
            SemanticType::Array(elem, len) => {
                let new_elem = self.clone_type_from(elem, source_ctx, lookup_sym);
                self.intern(SemanticType::Array(new_elem, len))
            }
            SemanticType::Slice(elem) => {
                let new_elem = self.clone_type_from(elem, source_ctx, lookup_sym);
                self.intern(SemanticType::Slice(new_elem))
            }
            SemanticType::Function { params, return_type } => {
                let new_params: Vec<_> = params.iter().map(|&p| self.clone_type_from(p, source_ctx, lookup_sym)).collect();
                let new_ret = self.clone_type_from(return_type, source_ctx, lookup_sym);
                self.intern(SemanticType::Function { params: new_params, return_type: new_ret })
            }
            SemanticType::Pointer(mutability, inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, lookup_sym);
                self.intern(SemanticType::Pointer(mutability, new_inner))
            }
            SemanticType::Reference(lt, mutability, inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, lookup_sym);
                self.intern(SemanticType::Reference(lt, mutability, new_inner))
            }
            SemanticType::Box(inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, lookup_sym);
                self.intern(SemanticType::Box(new_inner))
            }
            SemanticType::GenericParam(sym) => {
                let new_sym = lookup_sym(sym);
                self.intern(SemanticType::GenericParam(new_sym))
            }
            SemanticType::InferenceVar(v) => {
                if let Some(&bound) = source_ctx.inference_bindings.get(&v) {
                    self.clone_type_from(bound, source_ctx, lookup_sym)
                } else {
                    self.new_inference_var()
                }
            }
            SemanticType::Void => self.intern(SemanticType::Void),
            SemanticType::Never => self.intern(SemanticType::Never),
            SemanticType::Error => self.intern(SemanticType::Error),
            SemanticType::Closure(expr_id, params, ret) => {
                let new_params = params.iter().map(|p| self.clone_type_from(*p, source_ctx, lookup_sym)).collect();
                let new_ret = self.clone_type_from(ret, source_ctx, lookup_sym);
                self.intern(SemanticType::Closure(expr_id, new_params, new_ret))
            }
            SemanticType::DynTrait(sym) => {
                let new_sym = lookup_sym(sym);
                self.intern(SemanticType::DynTrait(new_sym))
            }
            SemanticType::Future(inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, lookup_sym);
                self.intern(SemanticType::Future(new_inner))
            }
            SemanticType::Range(inner) => {
                let new_inner = self.clone_type_from(inner, source_ctx, lookup_sym);
                self.intern(SemanticType::Range(new_inner))
            }
        }
    }
}
