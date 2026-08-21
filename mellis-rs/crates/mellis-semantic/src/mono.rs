use mellis_ast::{AstArena, Item, Stmt, Expr, Decl, DeclId};
use crate::{SemanticContext, ty::{TypeSubst, SemanticTypeId, SemanticType}};
use std::collections::{HashSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoInstance {
    pub decl_id: DeclId,
    // We sort the subst keys to make it Hashable, or just use a stable struct
    // For simplicity in this compiler, we'll store a serialized string of types, or just the subst map
    // Because HashMap doesn't implement Hash, we'll use a Vec of pairs sorted by string
    pub subst: Vec<(String, SemanticTypeId)>,
}

pub struct Monomorphizer<'a> {
    ctx: &'a SemanticContext,
    arena: &'a AstArena,
    pub instances: HashSet<MonoInstance>,
}

impl<'a> Monomorphizer<'a> {
    pub fn new(ctx: &'a SemanticContext, arena: &'a AstArena) -> Self {
        Self {
            ctx,
            arena,
            instances: HashSet::new(),
        }
    }

    pub fn run(&mut self, items: &[Item]) {
        for item in items {
            self.visit_item(item);
        }
    }

    fn visit_item(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { body, generic_params, .. } => {
                        // Only visit the body of non-generic functions as starting points.
                        // Generic functions are only visited when instantiated.
                        if generic_params.is_empty() {
                            if let Some(body_stmt) = body {
                                self.visit_stmt(body_stmt);
                            }
                            // Add to instances with empty subst
                            self.instances.insert(MonoInstance {
                                decl_id: *decl_id,
                                subst: vec![],
                            });
                        }
                    }
                    _ => {}
                }
            }
            Item::Stmt(stmt_id) => {
                self.visit_stmt(stmt_id);
            }
        }
    }

    fn visit_stmt(&mut self, stmt_id: &mellis_ast::StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        match stmt {
            Stmt::Block { body, tail_expr } => {
                for item in body {
                    self.visit_item(item);
                }
                if let Some(expr) = tail_expr {
                    self.visit_expr(expr);
                }
            }
            Stmt::Expr { expr, .. } => {
                self.visit_expr(expr);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.visit_expr(condition);
                self.visit_stmt(then_branch);
                if let Some(else_br) = else_branch {
                    self.visit_stmt(else_br);
                }
            }
            Stmt::While { condition, body, .. } => {
                self.visit_expr(condition);
                self.visit_stmt(body);
            }
            Stmt::For { init, cond, step, body, iterable, .. } => {
                if let Some(i) = init { self.visit_item(i); }
                if let Some(c) = cond { self.visit_expr(c); }
                if let Some(s) = step { self.visit_expr(s); }
                if let Some(it) = iterable { self.visit_expr(it); }
                self.visit_stmt(body);
            }
            Stmt::Return { value } => {
                if let Some(val) = value {
                    self.visit_expr(val);
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr_id: &mellis_ast::ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        match expr {
            Expr::Call { callee, generic_args, args } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(&arg.value);
                }
                
                // If it's a generic function, instantiate it
                if let Expr::Identifier { segments, .. } = &self.arena.exprs[callee.0 as usize] {
                    if let Some(sym_id) = self.ctx.tables.expr_symbols.get(callee) {
                        if let Some(decl_id) = self.ctx.tables.symbol_decls.get(sym_id) {
                            if let Decl::Function { generic_params, .. } = &self.arena.decls[decl_id.0 as usize] {
                                if !generic_params.is_empty() && generic_params.len() == generic_args.len() {
                                    let mut subst_vec = Vec::new();
                                    // In a real compiler, we need to extract string names of generic parameters
                                    // For simplicity here, we assume generic_params names are "T", "U", etc.
                                    // But actually we have to lower `generic_args` to `SemanticTypeId`
                                    for (i, param) in generic_params.iter().enumerate() {
                                        let name = format!("T{}", i); // stub
                                        if let Some(sem_ty) = self.ctx.tables.ast_type_to_semantic.get(&generic_args[i]) {
                                            subst_vec.push((name, *sem_ty));
                                        }
                                    }
                                    subst_vec.sort_by(|a, b| a.0.cmp(&b.0));
                                    
                                    let instance = MonoInstance {
                                        decl_id: *decl_id,
                                        subst: subst_vec,
                                    };
                                    
                                    if self.instances.insert(instance) {
                                        // Recursively visit the instantiated generic function body
                                        // Wait, the generic function body needs to be visited!
                                        if let Decl::Function { body: Some(body_stmt), .. } = &self.arena.decls[decl_id.0 as usize] {
                                            self.visit_stmt(body_stmt);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Assign { lvalue, value, .. } => {
                self.visit_expr(lvalue);
                self.visit_expr(value);
            }
            Expr::Member { object, .. } => {
                self.visit_expr(object);
            }
            Expr::StructInit { fields, .. } => {
                for field in fields {
                    self.visit_expr(&field.value);
                }
                // (Instantiate struct generic types if needed, though usually they don't produce code bodies,
                // but we might need them for sizes in LLVM codegen)
            }
            _ => {}
        }
    }
}
