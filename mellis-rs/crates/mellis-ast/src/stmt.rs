use crate::{DeclId, ExprId, PatId};
use mellis_common::Span;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ForKind {
    ForEach,
    CStyle,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum Item {
    Decl(DeclId),
    Stmt(crate::StmtId),
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum Stmt {
    Block {
        body: Vec<Item>,
        tail_expr: Option<ExprId>,
    },
    Expr {
        expr: ExprId,
        has_semicolon: bool,
    },
    If {
        condition: ExprId,
        then_branch: crate::StmtId, // BlockStmt
        else_branch: Option<crate::StmtId>,
    },
    While {
        label: Option<Span>,
        condition: ExprId,
        body: crate::StmtId, // BlockStmt
    },
    For {
        kind: ForKind,
        label: Option<Span>,
        pattern: Option<PatId>,
        iterable: Option<ExprId>,
        init: Option<Item>,
        cond: Option<ExprId>,
        step: Option<ExprId>,
        body: crate::StmtId, // BlockStmt
    },
    Return {
        value: Option<ExprId>,
    },
    Break {
        label: Option<Span>,
    },
    Continue {
        label: Option<Span>,
    },
    Unsafe {
        body: crate::StmtId, // BlockStmt
    },
    Comptime {
        body: crate::StmtId, // BlockStmt
    },
}
