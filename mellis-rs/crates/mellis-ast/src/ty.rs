use crate::{expr::TokenTree, ExprId, MacroDelimiter, TypeId};
use mellis_common::Span;
use mellis_lexer::BuiltinKind;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct AssociatedBinding {
    pub name: Span,
    pub ty: TypeId,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
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
    Typeof {
        expr: ExprId,
    },
    MacroCall {
        name: Span,
        path: Vec<Span>,
        delimiter: MacroDelimiter,
        args: Vec<TokenTree>,
        raw_tokens: Vec<mellis_lexer::Token>,
        span: Span,
    },
}
