use crate::{DeclId, ExprId, PatId, StmtId, TypeId};
use mellis_common::Span;
use mellis_lexer::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Deref,
    Ref,
    RefMut,
    PostInc,
    PostDec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct CallArg {
    pub label: Option<Span>,
    pub value: ExprId,
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: Span,
    pub value: ExprId,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: PatId,
    pub body: StmtId, // Must be BlockStmt
}

#[derive(Debug, Clone)]
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
    Typeof {
        expr: ExprId,
    },
}
