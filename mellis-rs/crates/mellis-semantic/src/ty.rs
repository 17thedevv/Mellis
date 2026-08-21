use mellis_common::ids::SymbolId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticTypeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BuiltinType {
    Int,
    Float,
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

pub type TypeSubst = HashMap<String, SemanticTypeId>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SemanticType {
    Primitive(BuiltinType),
    Struct(SymbolId, Vec<SemanticTypeId>),
    Enum(SymbolId, Vec<SemanticTypeId>),
    Tuple(Vec<SemanticTypeId>),
    Array(SemanticTypeId, u64),
    Slice(SemanticTypeId),
    Function { params: Vec<SemanticTypeId>, return_type: SemanticTypeId },
    Pointer(SemanticTypeId),
    Reference(LifetimeId, Mutability, SemanticTypeId),
    Void,
    Never,
    Error,
    InferenceVar(u32),
    Generic(String),
}

pub struct TypeContext {
    types: Vec<SemanticType>,
    // Hash-consing map to reuse types
    type_interner: HashMap<SemanticType, SemanticTypeId>,
    next_inference_var: u32,
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            types: Vec::new(),
            type_interner: HashMap::new(),
            next_inference_var: 0,
        };
        // Pre-populate standard types so they have fixed IDs if we want
        ctx.intern(SemanticType::Void);
        ctx.intern(SemanticType::Error);
        ctx.intern(SemanticType::Never);
        ctx.intern(SemanticType::Primitive(BuiltinType::Int));
        ctx.intern(SemanticType::Primitive(BuiltinType::Bool));
        ctx.intern(SemanticType::Primitive(BuiltinType::Float));
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

    pub fn get(&self, id: SemanticTypeId) -> &SemanticType {
        &self.types[id.0 as usize]
    }

    pub fn new_inference_var(&mut self) -> SemanticTypeId {
        let id = self.next_inference_var;
        self.next_inference_var += 1;
        self.intern(SemanticType::InferenceVar(id))
    }
    
    pub fn subst(&mut self, id: SemanticTypeId, subst: &TypeSubst) -> SemanticTypeId {
        let ty = self.get(id).clone();
        match ty {
            SemanticType::Generic(name) => {
                if let Some(&new_id) = subst.get(&name) {
                    new_id
                } else {
                    id
                }
            }
            SemanticType::Struct(sym, args) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.subst(a, subst)).collect();
                self.intern(SemanticType::Struct(sym, new_args))
            }
            SemanticType::Enum(sym, args) => {
                let new_args: Vec<_> = args.iter().map(|&a| self.subst(a, subst)).collect();
                self.intern(SemanticType::Enum(sym, new_args))
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
            SemanticType::Pointer(inner) => {
                let new_inner = self.subst(inner, subst);
                self.intern(SemanticType::Pointer(new_inner))
            }
            SemanticType::Reference(lt, mutability, inner) => {
                let new_inner = self.subst(inner, subst);
                self.intern(SemanticType::Reference(lt, mutability, new_inner))
            }
            _ => id, // Primitive, Void, Error, Never, InferenceVar
        }
    }
}
