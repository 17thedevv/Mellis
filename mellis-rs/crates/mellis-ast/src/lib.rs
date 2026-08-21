pub mod decl;
pub mod expr;
pub mod pat;
pub mod stmt;
pub mod ty;

pub use decl::*;
pub use expr::*;
pub use pat::*;
pub use stmt::*;
pub use ty::*;

use mellis_common::ids::{FileId, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StmtId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeclId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatId(pub u32);

#[derive(Debug, Default)]
pub struct AstArena {
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Stmt>,
    pub decls: Vec<Decl>,
    pub types: Vec<Type>,
    pub pats: Vec<Pattern>,
}

impl AstArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = self.exprs.len() as u32;
        self.exprs.push(expr);
        ExprId(id)
    }

    pub fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        let id = self.stmts.len() as u32;
        self.stmts.push(stmt);
        StmtId(id)
    }

    pub fn alloc_decl(&mut self, decl: Decl) -> DeclId {
        let id = self.decls.len() as u32;
        self.decls.push(decl);
        DeclId(id)
    }

    pub fn alloc_type(&mut self, ty: Type) -> TypeId {
        let id = self.types.len() as u32;
        self.types.push(ty);
        TypeId(id)
    }

    pub fn alloc_pat(&mut self, pat: Pattern) -> PatId {
        let id = self.pats.len() as u32;
        self.pats.push(pat);
        PatId(id)
    }
}
