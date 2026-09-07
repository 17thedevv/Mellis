#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Local,
    External,
}

use crate::{ExprId, PatId, StmtId, TypeId};
use mellis_common::Span;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Internal,
    Public,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct AnnotationArg {
    pub key: Option<Span>,
    pub value: ExprId,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Annotation {
    pub name: Span,
    pub args: Vec<AnnotationArg>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum GenericParamKind {
    Type,
    Lifetime,
    Const,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GenericParam {
    pub name: Span,
    pub kind: GenericParamKind,
    pub bounds: Vec<TypeId>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct StructField {
    pub name: Span,
    pub ty: TypeId,
    pub visibility: Visibility,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct EnumVariant {
    pub annotations: Vec<Annotation>,
    pub name: Span,
    pub fields: Vec<crate::DeclId>, // ParamDecl
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum Decl {
    Var {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        pattern: Option<PatId>,
        type_annot: Option<TypeId>,
        initializer: Option<ExprId>,
        is_mutable: bool,
        is_const: bool,
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
        /// Explicit lifetime signature via `life_from` and `where outlives` clauses.
        /// This is parsed after the return type.
        lifetime_signature: crate::FnLifetimeSignature,
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

    Import {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        kind: ImportKind,
        name: Span,
    },
    Module {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        items: Vec<crate::DeclId>,
    },
    /// `using std::collections as col;` — local namespace alias.
    /// path: the segments of the namespace path (e.g., [std, collections])
    /// alias: the local alias identifier (e.g., col)
    /// span: the full declaration span for diagnostics
    Using {
        path: Vec<Span>,
        alias: Span,
        span: Span,
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
    Macro {
        annotations: Vec<Annotation>,
        visibility: Visibility,
        name: Span,
        rules: Vec<MacroRule>,
    },
}

impl Decl {
    pub fn annotations(&self) -> &[Annotation] {
        match self {
            Decl::Var { annotations, .. } => annotations,
            Decl::Param { annotations, .. } => annotations,
            Decl::Function { annotations, .. } => annotations,
            Decl::Struct { annotations, .. } => annotations,
            Decl::Enum { annotations, .. } => annotations,
            Decl::Trait { annotations, .. } => annotations,
            Decl::Impl { annotations, .. } => annotations,
            Decl::Import { annotations, .. } => annotations,
            Decl::Module { annotations, .. } => annotations,
            Decl::TypeAlias { annotations, .. } => annotations,
            Decl::Macro { annotations, .. } => annotations,
            Decl::Extern { annotations, .. } => annotations,
            Decl::Using { .. } => &[],
        }
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum FragmentKind {
    Expr,
    Ident,
    Ty,
    Stmt,
    Block,
    Item,
    Tt,
    Pat,
    Path,
    Lifetime,
    Meta,
    Literal,
    Vis,
}

pub type MacroFragment = FragmentKind;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum RepetitionKind {
    ZeroOrMore, // *
    OneOrMore,  // +
    Optional,   // ?
}

/// A tree-oriented element of a macro pattern.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum MatcherElement {
    /// A macro metavariable capture, e.g. `@x: expr`
    MetaVar {
        name: Span,
        fragment: FragmentKind,
        span: Span,
    },
    /// A delimited group of matcher elements: `(...)`, `[...]`, `{...}`
    Group {
        delimiter: crate::MacroDelimiter,
        elements: Vec<MatcherElement>,
        span: Span,
    },
    /// A literal token inside a matcher, e.g. `,`, `+`, `;`, identifier, keyword, etc.
    Leaf {
        token: mellis_lexer::Token,
    },
    /// Extension point for future repetition syntax (e.g. `@(...)*`)
    Repetition {
        elements: Vec<MatcherElement>,
        separator: Option<mellis_lexer::TokenKind>,
        kind: RepetitionKind,
        span: Span,
    },
}

/// A macro rule pattern enclosing matcher elements within delimiters.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MacroPattern {
    pub delimiter: crate::MacroDelimiter,
    pub elements: Vec<MatcherElement>,
    pub span: Span,
}

/// A tree-oriented element of a macro transcriber template.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum TranscriberElement {
    /// Reference to a captured metavariable: `@x`
    MetaVar {
        name: Span,
        span: Span,
    },
    /// A delimited group of transcriber elements: `(...)`, `[...]`, `{...}`
    Group {
        delimiter: crate::MacroDelimiter,
        elements: Vec<TranscriberElement>,
        span: Span,
    },
    /// A literal token inside the template
    Leaf {
        token: mellis_lexer::Token,
    },
    /// Extension point for future repetition expansion
    Repetition {
        elements: Vec<TranscriberElement>,
        separator: Option<mellis_lexer::TokenKind>,
        kind: RepetitionKind,
        span: Span,
    },
}

/// A macro rule transcriber body enclosing template elements within delimiters.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MacroTranscriber {
    pub delimiter: crate::MacroDelimiter,
    pub elements: Vec<TranscriberElement>,
    pub span: Span,
}

/// A single rule in a macro: `pattern => transcriber`
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MacroRule {
    pub pattern: MacroPattern,
    pub transcriber: MacroTranscriber,
    /// Flat matcher list kept for backwards-compatibility with expansion engine
    pub matchers: Vec<MacroMatcher>,
    /// Flat template tokens kept for backwards-compatibility with expansion engine
    pub template_tokens: Vec<mellis_lexer::Token>,
    pub span: Span,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MacroMatcher {
    pub name: Span,
    pub fragment: FragmentKind,
    pub separator: Option<mellis_lexer::TokenKind>,
    pub repetition: Option<RepetitionKind>,
}

/// A declarative macro declaration
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MacroDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub name: Span,
    pub rules: Vec<MacroRule>,
    pub span: Span,
}
