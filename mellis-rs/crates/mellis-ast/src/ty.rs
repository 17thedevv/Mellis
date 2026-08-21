use crate::{ExprId, TypeId};
use mellis_common::Span;
use mellis_lexer::BuiltinKind;

#[derive(Debug, Clone)]
pub struct AssociatedBinding {
    pub name: Span,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub enum Type {
    Builtin(BuiltinKind),
    Lifetime(Span),
    Named {
        segments: Vec<Span>,
        generic_args: Vec<TypeId>,
        associated_bindings: Vec<AssociatedBinding>,
    },
    Reference {
        is_mutable: bool,
        lifetime: Option<TypeId>, // Lifetime
        inner: TypeId,
    },
    Pointer {
        is_mutable: bool,
        inner: TypeId,
    },
    Array {
        element_type: TypeId,
        size: ExprId,
    },
    Slice {
        inner: TypeId,
    },
    Tuple {
        elements: Vec<TypeId>,
    },
    Function {
        params: Vec<TypeId>,
        return_type: Option<TypeId>,
        is_unsafe: bool,
    },
    Never,
    TraitObject {
        trait_type: TypeId,
    },
}
