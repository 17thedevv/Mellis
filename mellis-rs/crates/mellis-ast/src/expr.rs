use crate::{DeclId, ExprId, PatId, StmtId, TypeId};
use mellis_common::Span;
use mellis_lexer::Token;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LogicAnd,
    LogicOr,
    BitAnd,
    BitOr,
    BitXor,
    LShift,
    RShift,
    Range,
    RangeInc,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Deref,
    DerefMut,
    Ref,
    RefMut,
    PostInc,
    PostDec,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    LShiftAssign,
    RShiftAssign,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CallArg {
    pub label: Option<Span>,
    pub value: ExprId,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct FieldInit {
    pub name: Span,
    pub value: ExprId,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MatchArm {
    pub pattern: PatId,
    pub body: StmtId, // Must be BlockStmt
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum Expr {
    Literal(Token), // Integer, Float, Char, Str, etc.
    Identifier {
        segments: Vec<Span>,
        generic_args: Vec<TypeId>,
    },
    Binary {
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },
    Unary {
        op: UnaryOp,
        operand: ExprId,
    },
    Assign {
        op: AssignOp,
        lvalue: ExprId,
        value: ExprId,
    },
    Call {
        callee: ExprId,
        generic_args: Vec<TypeId>,
        args: Vec<CallArg>,
    },
    MethodCall {
        object: ExprId,
        method_name: Span,
        generic_args: Vec<TypeId>,
        args: Vec<CallArg>,
    },
    Index {
        base: ExprId,
        index: ExprId,
    },
    Member {
        object: ExprId,
        member: Span,
    },
    TupleIndex {
        object: ExprId,
        index: u32,
    },
    Cast {
        expr: ExprId,
        target_type: TypeId,
    },
    ArrayLiteral {
        elements: Vec<ExprId>,
    },
    TupleLiteral {
        elements: Vec<ExprId>,
    },
    StructInit {
        path: Vec<Span>,
        generic_args: Vec<TypeId>,
        fields: Vec<FieldInit>,
    },
    Match {
        match_span: Span,
        subject: ExprId,
        arms: Vec<MatchArm>,
    },
    Lambda {
        params: Vec<DeclId>, // ParamDecl
        return_type: Option<TypeId>,
        body: StmtId, // BlockStmt
        is_move: bool,
    },
    Try {
        expr: ExprId,
        try_span: Span,
    },
    Await {
        expr: ExprId,
    },
    Sizeof {
        target_type: TypeId,
    },
    Alignof {
        target_type: TypeId,
    },
    MacroCall {
        name: Span,
        path: Vec<Span>,
        delimiter: MacroDelimiter,
        args: Vec<TokenTree>,
        raw_tokens: Vec<Token>,
        span: Span,
    },
    Comptime {
        body: StmtId,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum MacroDelimiter {
    Paren,
    Bracket,
    Brace,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum TokenTree {
    Group {
        delimiter: MacroDelimiter,
        tokens: Vec<TokenTree>,
        span: Span,
    },
    Leaf {
        token: Token,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MacroCall {
    pub name: Span,
    pub path: Vec<Span>,
    pub delimiter: MacroDelimiter,
    pub args: Vec<TokenTree>,
    pub raw_tokens: Vec<Token>,
    pub span: Span,
}


