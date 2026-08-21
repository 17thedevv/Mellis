use crate::{ExprId, PatId, StmtId, TypeId};
use mellis_common::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Internal,
    Public,
}

#[derive(Debug, Clone)]
pub struct AnnotationArg {
    pub key: Option<Span>,
    pub value: ExprId,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: Span,
    pub args: Vec<AnnotationArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericParamKind {
    Type,
    Lifetime,
    Const,
}

#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: Span,
    pub kind: GenericParamKind,
    pub bounds: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: Span,
    pub ty: TypeId,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Span,
    pub fields: Vec<crate::DeclId>, // ParamDecl
}

#[derive(Debug, Clone)]
pub struct UseTree {
    pub segments: Vec<Span>,
    pub alias: Option<Span>,
    pub is_glob: bool,
    pub children: Vec<UseTree>,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Var {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        pattern: Option<PatId>,
        type_annot: Option<TypeId>,
        initializer: Option<ExprId>,
        is_mutable: bool,
    },
    Param {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        ty: Option<TypeId>,
        is_variadic: bool,
        is_self: bool,
    },
    Function {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        generic_params: Vec<GenericParam>,
        params: Vec<crate::DeclId>, // ParamDecl
        return_type: Option<TypeId>,
        body: Option<StmtId>, // BlockStmt
        is_async: bool,
        is_comptime: bool,
        is_variadic: bool,
        is_unsafe: bool,
        is_intrinsic: bool,
    },
    Struct {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        generic_params: Vec<GenericParam>,
        fields: Vec<StructField>,
    },
    Enum {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        generic_params: Vec<GenericParam>,
        variants: Vec<EnumVariant>,
    },
    Trait {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        generic_params: Vec<GenericParam>,
        associated_types: Vec<crate::DeclId>, // TypeAliasDecl
        methods: Vec<crate::DeclId>,          // FunctionDecl
    },
    Impl {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        generic_params: Vec<GenericParam>,
        self_type: TypeId,
        trait_type: Option<TypeId>,
        associated_types: Vec<crate::DeclId>, // TypeAliasDecl
        methods: Vec<crate::DeclId>,          // FunctionDecl
    },
    Mod {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        decls: Vec<crate::DeclId>,
        is_outlined: bool,
    },
    Use {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        tree: UseTree,
    },
    Extern {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        func: crate::DeclId, // FunctionDecl
    },
    TypeAlias {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        generic_params: Vec<GenericParam>,
        bounds: Vec<TypeId>,
        aliased_type: Option<TypeId>,
    },
}
