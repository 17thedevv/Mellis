use crate::{SemanticContext, ScopeId, SymbolKind};
use mellis_ast::{AstArena, Item, Stmt, Expr, Decl, Pattern};

pub struct Resolver<'a> {
    ctx: &'a mut SemanticContext,
    arena: &'a AstArena,
    source: &'a str,
    current_scope: ScopeId,
}

impl<'a> Resolver<'a> {
    pub fn new(ctx: &'a mut SemanticContext, arena: &'a AstArena, source: &'a str) -> Self {
        // Assume global scope is 0
        let global_scope = ScopeId(0);
        Self {
            ctx,
            arena,
            source,
            current_scope: global_scope,
        }
    }
    
    pub fn enter_scope(&mut self, kind: crate::symbol::ScopeKind) -> ScopeId {
        let new_scope = self.ctx.symbol_table.create_scope(kind, Some(self.current_scope));
        self.current_scope = new_scope;
        new_scope
    }
    
    pub fn exit_scope(&mut self) {
        if let Some(parent) = self.ctx.symbol_table.scopes[self.current_scope.0 as usize].parent {
            self.current_scope = parent;
        }
    }

    pub fn resolve_items(&mut self, items: &[Item]) {
        for item in items {
            self.resolve_item(item);
        }
    }

    fn resolve_item(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { name, params, body, visibility, .. } => {
                        let name_str = self.source[name.start as usize..name.end as usize].to_string();
                        
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            SymbolKind::Function,
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        
                        self.enter_scope(crate::symbol::ScopeKind::Function);
                        
                        for param_id in params {
                            if let Decl::Param { name: p_name, visibility: p_vis, .. } = &self.arena.decls[param_id.0 as usize] {
                                let p_name_str = format!("param_{}_{}", p_name.start, p_name.end);
                                let p_sym_id = self.ctx.symbol_table.declare_symbol(
                                    p_name_str,
                                    SymbolKind::Variable,
                                    self.current_scope,
                                    *p_name,
                                    Some(*param_id),
                                    *p_vis,
                                );
                                self.ctx.tables.decl_symbols.insert(*param_id, p_sym_id);
                                self.ctx.tables.symbol_decls.insert(p_sym_id, *param_id);
                            }
                        }
                        
                        if let Some(body_stmt) = body {
                            self.resolve_stmt(body_stmt);
                        }
                        
                        self.exit_scope();
                    }
                    Decl::Var { name, initializer, visibility, is_mutable, pattern, .. } => {
                        if let Some(init) = initializer {
                            self.resolve_expr(init);
                        }
                        
                        // We need to resolve the pattern or use the fallback name.
                        let name_str = self.source[name.start as usize..name.end as usize].to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            if *is_mutable { SymbolKind::Variable } else { SymbolKind::Constant },
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        if let Some(pattern) = pattern {
                            self.resolve_pattern(pattern, *visibility, *is_mutable);
                        }
                    }
                    Decl::Struct { name, fields: _, visibility, .. } => {
                        let name_str = self.source[name.start as usize..name.end as usize].to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            SymbolKind::Struct,
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                    }
                    Decl::Extern { func, visibility, .. } => {
                        let item = Item::Decl(*func);
                        self.resolve_item(&item);
                    }
                    Decl::Enum { name, variants, visibility, .. } => {
                        let name_str = self.source[name.start as usize..name.end as usize].to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str.clone(),
                            SymbolKind::Struct, // Enums use Struct kind for now
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        
                        // Declare each variant as a symbol accessible via EnumName::VariantName
                        for (idx, variant) in variants.iter().enumerate() {
                            let v_name_str = self.source[variant.name.start as usize..variant.name.end as usize].to_string();
                            let variant_full_name = format!("{}::{}", name_str, v_name_str);
                            let _v_sym_id = self.ctx.symbol_table.declare_symbol(
                                variant_full_name,
                                SymbolKind::EnumVariant(idx as u32),
                                self.current_scope,
                                variant.name,
                                Some(*decl_id), // Point to the Enum decl
                                *visibility,
                            );
                        }
                    }
                    Decl::Trait { name, methods, visibility, .. } => {
                        let name_str = self.source[name.start as usize..name.end as usize].to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            SymbolKind::Struct, // Traits use Struct kind for now
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        
                        for method_id in methods {
                            let item = Item::Decl(*method_id);
                            self.resolve_item(&item);
                        }
                    }
                    Decl::Impl { methods, .. } => {
                        for method_id in methods {
                            let item = Item::Decl(*method_id);
                            self.resolve_item(&item);
                        }
                    }
                    _ => {}
                }
            }
            Item::Stmt(stmt_id) => {
                self.resolve_stmt(stmt_id);
            }
        }
    }
    
    fn resolve_stmt(&mut self, stmt_id: &mellis_ast::StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        match stmt {
            Stmt::Block { body, tail_expr } => {
                self.enter_scope(crate::symbol::ScopeKind::Block);
                self.resolve_items(body);
                if let Some(expr) = tail_expr {
                    self.resolve_expr(expr);
                }
                self.exit_scope();
            }
            Stmt::Expr { expr, .. } => {
                self.resolve_expr(expr);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.resolve_expr(condition);
                self.resolve_stmt(then_branch);
                if let Some(else_br) = else_branch {
                    self.resolve_stmt(else_br);
                }
            }
            Stmt::While { condition, body, .. } => {
                self.resolve_expr(condition);
                self.resolve_stmt(body);
            }
            Stmt::For { init, cond, step, body, iterable, .. } => {
                self.enter_scope(crate::symbol::ScopeKind::Block);
                if let Some(item) = init {
                    self.resolve_item(item);
                }
                if let Some(iter) = iterable {
                    self.resolve_expr(iter);
                }
                if let Some(c) = cond {
                    self.resolve_expr(c);
                }
                if let Some(s) = step {
                    self.resolve_expr(s);
                }
                self.resolve_stmt(body);
                self.exit_scope();
            }
            Stmt::Return { value } => {
                if let Some(val) = value {
                    self.resolve_expr(val);
                }
            }
            _ => {}
        }
    }

    fn resolve_pattern(&mut self, pat_id: &mellis_ast::PatId, visibility: mellis_ast::Visibility, is_mutable: bool) {
        match &self.arena.pats[pat_id.0 as usize] {
            Pattern::Identifier { segments } => {
                if let Some(name) = segments.last() {
                    let name_str = self.source[name.start as usize..name.end as usize].to_string();
                    
                    // Check if it's an enum variant
                    if let Some(existing_sym_id) = self.ctx.symbol_table.lookup(&name_str, self.current_scope) {
                        let sym = self.ctx.symbol_table.get_symbol(existing_sym_id);
                        if matches!(sym.kind, SymbolKind::EnumVariant(_)) {
                            self.ctx.tables.pat_symbols.insert(*pat_id, existing_sym_id);
                            return;
                        }
                    }
                    
                    let sym_id = self.ctx.symbol_table.declare_symbol(
                        name_str,
                        if is_mutable { SymbolKind::Variable } else { SymbolKind::Constant },
                        self.current_scope,
                        *name,
                        None,
                        visibility,
                    );
                    self.ctx.tables.pat_symbols.insert(*pat_id, sym_id);
                }
            }
            Pattern::Tuple { elements, .. } | Pattern::Enum { fields: elements, .. } => {
                for element in elements {
                    self.resolve_pattern(element, visibility, is_mutable);
                }
            }
            Pattern::Struct { fields, .. } => {
                for field in fields {
                    if let Some(pattern) = field.pattern {
                        self.resolve_pattern(&pattern, visibility, is_mutable);
                    }
                }
            }
            Pattern::Literal(_) | Pattern::Wildcard => {}
        }
    }
    
    fn resolve_expr(&mut self, expr_id: &mellis_ast::ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        match expr {
            Expr::Identifier { segments, .. } => {
                if !segments.is_empty() {
                    let mut name_str = String::new();
                    for (i, seg) in segments.iter().enumerate() {
                        if i > 0 { name_str.push_str("::"); }
                        name_str.push_str(&self.source[seg.start as usize..seg.end as usize]);
                    }
                    if let Some(sym_id) = self.ctx.symbol_table.lookup(&name_str, self.current_scope) {
                        self.ctx.tables.expr_symbols.insert(*expr_id, sym_id);
                    } else {
                        // We use dummy names so lookups will fail for non-local standard names unless we use SourceManager
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            Expr::Unary { operand, .. } => {
                self.resolve_expr(operand);
            }
            Expr::Call { callee, args, .. } => {
                self.resolve_expr(callee);
                for arg in args {
                    self.resolve_expr(&arg.value);
                }
            }
            Expr::Assign { lvalue, value, .. } => {
                self.resolve_expr(lvalue);
                self.resolve_expr(value);
            }
            Expr::MethodCall { object, args, .. } => {
                self.resolve_expr(object);
                for arg in args {
                    self.resolve_expr(&arg.value);
                }
            }
            Expr::Member { object, .. } => {
                self.resolve_expr(object);
            }
            Expr::StructInit { fields, .. } => {
                for field in fields {
                    self.resolve_expr(&field.value);
                }
            }
            Expr::Index { base, index } => {
                self.resolve_expr(base);
                self.resolve_expr(index);
            }
            Expr::TupleIndex { object, .. } => {
                self.resolve_expr(object);
            }
            Expr::Cast { expr: e, .. } => {
                self.resolve_expr(e);
            }
            Expr::Match { subject, arms } => {
                self.resolve_expr(subject);
                for arm in arms {
                    self.enter_scope(crate::symbol::ScopeKind::Block);
                    self.resolve_pattern(&arm.pattern, mellis_ast::Visibility::Private, true);
                    self.resolve_stmt(&arm.body);
                    self.exit_scope();
                }
            }
            Expr::Lambda { params, body, .. } => {
                self.enter_scope(crate::symbol::ScopeKind::Function);
                for param_id in params {
                    if let Decl::Param { name: p_name, visibility: p_vis, .. } = &self.arena.decls[param_id.0 as usize] {
                        let p_name_str = self.source[p_name.start as usize..p_name.end as usize].to_string();
                        let p_sym_id = self.ctx.symbol_table.declare_symbol(
                            p_name_str,
                            SymbolKind::Variable,
                            self.current_scope,
                            *p_name,
                            Some(*param_id),
                            *p_vis,
                        );
                        self.ctx.tables.decl_symbols.insert(*param_id, p_sym_id);
                        self.ctx.tables.symbol_decls.insert(p_sym_id, *param_id);
                    }
                }
                self.resolve_stmt(body);
                self.exit_scope();
            }
            Expr::Try { expr: e } | Expr::Await { expr: e } => {
                self.resolve_expr(e);
            }
            Expr::Unary { operand, .. } => {
                self.resolve_expr(operand);
            }
            Expr::ArrayLiteral { elements } | Expr::TupleLiteral { elements } => {
                for e in elements {
                    self.resolve_expr(e);
                }
            }
            _ => {}
        }
    }
}
