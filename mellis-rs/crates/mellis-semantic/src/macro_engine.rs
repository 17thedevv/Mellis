use std::collections::HashMap;
use mellis_ast::{
    AstArena, Decl, DeclId, Expr, ExprId, FragmentKind, Item, MacroDelimiter, MacroFragment,
    MacroMatcher, MacroRule, MatcherElement, RepetitionKind, Stmt, StmtId, TranscriberElement,
    Type, TypeId,
};
use mellis_common::diagnostic::Diagnostic;
use mellis_common::ids::{FileId, Span, SyntaxContext};
use mellis_lexer::{Token, TokenKind};
use mellis_parser::Parser;
use crate::symbol::{ScopeId, SymbolTable};
use crate::semantic_tables::SemanticTables;

#[derive(Debug, Clone)]
pub enum CapturedFragment {
    Single(Vec<Token>),
    Repeated(Vec<CapturedFragment>),
}


#[derive(Debug, Clone)]
pub struct MatchFailure {
    pub pos: usize,
    pub expected: String,
    pub span: Span,
}

pub struct MacroEngine<'a> {
    pub arena: &'a mut AstArena,
    pub source_manager: &'a mellis_common::source::SourceManager,
    pub file_id: FileId,
    pub symbol_table: &'a SymbolTable,
    pub tables: &'a SemanticTables,
    pub current_scope: ScopeId,
    pub diagnostics: Vec<Diagnostic>,
    expansion_counter: u32,
    recursion_depth: u32,
}

impl<'a> MacroEngine<'a> {

    pub fn get_span_text(&self, span: mellis_common::ids::Span) -> &'a str {
        &self.source_manager.get_file(span.file_id).unwrap().source[span.start as usize..span.end as usize]
    }


    pub fn new(
        arena: &'a mut AstArena,
        source_manager: &'a mellis_common::source::SourceManager,
        file_id: FileId,
        symbol_table: &'a SymbolTable,
        tables: &'a SemanticTables,
    ) -> Self {
        Self {
            arena,
            source_manager,
            file_id,
            symbol_table,
            tables,
            current_scope: ScopeId(0),
            diagnostics: Vec::new(),
            expansion_counter: 0,
            recursion_depth: 0,
        }
    }

    pub fn expand_items(&mut self, items: Vec<Item>) -> Result<Vec<Item>, Vec<Diagnostic>> {
        let mut expanded_items = Vec::new();
        for item in items {
            expanded_items.extend(self.expand_block_item(item));
        }

        if self.diagnostics.is_empty() {
            Ok(expanded_items)
        } else {
            Err(self.diagnostics.clone())
        }
    }

    fn expand_decl(&mut self, decl_id: DeclId) {
        let decl = self.arena.decls[decl_id.0 as usize].clone();
        match decl {
            Decl::Function { name, params, return_type, body, .. } => {
                let name_str = self.get_span_text(name);
                let func_sym = self.symbol_table.lookup_exact(name_str, self.current_scope);
                let prev_scope = self.current_scope;
                if let Some(sym_id) = func_sym {
                    if let Some(inner) = self.symbol_table.symbols[sym_id.0 as usize].inner_scope {
                        self.current_scope = inner;
                    }
                }

                // Expand parameter types
                for &param_id in &params {
                    let old_ty = if let Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {
                        *ty
                    } else {
                        None
                    };
                    let new_ty = old_ty.map(|t| self.expand_type(t));
                    if let Decl::Param { ty, .. } = &mut self.arena.decls[param_id.0 as usize] {
                        *ty = new_ty;
                    }
                }

                // Expand return type
                let new_ret = return_type.map(|rt| self.expand_type(rt));
                if let Decl::Function { return_type: rt, .. } = &mut self.arena.decls[decl_id.0 as usize] {
                    *rt = new_ret;
                }

                if let Some(body_stmt) = body {
                    let _ = self.expand_stmt(body_stmt);
                }
                self.current_scope = prev_scope;
            }
            Decl::Var { type_annot, initializer, .. } => {
                let new_ty = type_annot.map(|t| self.expand_type(t));
                let new_init = initializer.map(|init_expr| self.expand_expr(init_expr));
                if let Decl::Var { type_annot: t, initializer: init, .. } = &mut self.arena.decls[decl_id.0 as usize] {
                    *t = new_ty;
                    *init = new_init;
                }
            }
            Decl::Struct { fields, .. } => {
                let new_fields = fields.into_iter().map(|mut f| {
                    f.ty = self.expand_type(f.ty);
                    f
                }).collect();
                if let Decl::Struct { fields: f, .. } = &mut self.arena.decls[decl_id.0 as usize] {
                    *f = new_fields;
                }
            }
            Decl::TypeAlias { bounds, aliased_type, .. } => {
                let new_bounds = bounds.into_iter().map(|b| self.expand_type(b)).collect();
                let new_aliased = aliased_type.map(|t| self.expand_type(t));
                if let Decl::TypeAlias { bounds: b, aliased_type: a, .. } = &mut self.arena.decls[decl_id.0 as usize] {
                    *b = new_bounds;
                    *a = new_aliased;
                }
            }
            Decl::Impl { self_type, trait_type, methods, .. } => {
                let new_self = self.expand_type(self_type);
                let new_trait = trait_type.map(|t| self.expand_type(t));
                if let Decl::Impl { self_type: s, trait_type: tr, .. } = &mut self.arena.decls[decl_id.0 as usize] {
                    *s = new_self;
                    *tr = new_trait;
                }
                for m in methods {
                    self.expand_decl(m);
                }
            }
            Decl::Trait { methods, .. } => {
                for m in methods {
                    self.expand_decl(m);
                }
            }
            Decl::Module { name, items, .. } => {
                let name_str = self.get_span_text(name);
                let mod_sym = self.symbol_table.lookup_exact(name_str, self.current_scope);
                let prev_scope = self.current_scope;
                if let Some(sym_id) = mod_sym {
                    if let Some(inner) = self.symbol_table.symbols[sym_id.0 as usize].inner_scope {
                        self.current_scope = inner;
                    }
                }
                for it in items {
                    self.expand_decl(it);
                }
                self.current_scope = prev_scope;
            }
            _ => {}
        }
    }

    fn expand_block_item(&mut self, item: Item) -> Vec<Item> {
        match item {
            Item::Decl(decl_id) => {
                self.expand_decl(decl_id);
                vec![Item::Decl(decl_id)]
            }
            Item::Stmt(stmt_id) => {
                let stmt = self.arena.stmts[stmt_id.0 as usize].clone();
                if let Stmt::Expr { expr, has_semicolon } = stmt {
                    let expr_data = &self.arena.exprs[expr.0 as usize];
                    if let Expr::MacroCall { name, path, raw_tokens, span, .. } = expr_data {
                        let name_copy = *name;
                        let path_copy = path.clone();
                        let raw_tokens_copy = raw_tokens.clone();
                        let call_span = *span;
                        let has_semi = has_semicolon;

                        if let Some(expanded_items) = self.expand_macro_call_items(name_copy, &path_copy, &raw_tokens_copy, call_span, has_semi) {
                            self.arena.exprs[expr.0 as usize] = Expr::TupleLiteral { elements: Vec::new() };
                            return expanded_items;
                        }
                    }
                }
                let expanded_stmts = self.expand_stmt(stmt_id);
                expanded_stmts.into_iter().map(Item::Stmt).collect()
            }
        }
    }

    fn expand_stmt(&mut self, stmt_id: StmtId) -> Vec<StmtId> {
        let stmt = self.arena.stmts[stmt_id.0 as usize].clone();
        match stmt {
            Stmt::Expr { expr, .. } => {
                let expanded_expr = self.expand_expr(expr);
                if let Stmt::Expr { expr: e, .. } = &mut self.arena.stmts[stmt_id.0 as usize] {
                    *e = expanded_expr;
                }
                vec![stmt_id]
            }
            Stmt::Block { body, tail_expr } => {
                let mut new_body = Vec::new();
                for item in body {
                    new_body.extend(self.expand_block_item(item));
                }
                let new_tail = tail_expr.map(|e| self.expand_expr(e));
                if let Stmt::Block { body: b, tail_expr: t } = &mut self.arena.stmts[stmt_id.0 as usize] {
                    *b = new_body;
                    *t = new_tail;
                }
                vec![stmt_id]
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let new_cond = self.expand_expr(condition);
                let _ = self.expand_stmt(then_branch);
                if let Some(else_b) = else_branch {
                    let _ = self.expand_stmt(else_b);
                }
                if let Stmt::If { condition: c, .. } = &mut self.arena.stmts[stmt_id.0 as usize] {
                    *c = new_cond;
                }
                vec![stmt_id]
            }
            Stmt::While { condition, body, .. } => {
                let new_cond = self.expand_expr(condition);
                let _ = self.expand_stmt(body);
                if let Stmt::While { condition: c, .. } = &mut self.arena.stmts[stmt_id.0 as usize] {
                    *c = new_cond;
                }
                vec![stmt_id]
            }
            Stmt::For { iterable, body, .. } => {
                let new_iter = iterable.map(|it| self.expand_expr(it));
                let _ = self.expand_stmt(body);
                if let Stmt::For { iterable: it, .. } = &mut self.arena.stmts[stmt_id.0 as usize] {
                    *it = new_iter;
                }
                vec![stmt_id]
            }
            Stmt::Return { value } => {
                let new_val = value.map(|e| self.expand_expr(e));
                if let Stmt::Return { value: v } = &mut self.arena.stmts[stmt_id.0 as usize] {
                    *v = new_val;
                }
                vec![stmt_id]
            }
            _ => vec![stmt_id],
        }
    }

    pub fn expand_expr(&mut self, expr_id: ExprId) -> ExprId {
        let expr = self.arena.exprs[expr_id.0 as usize].clone();
        match expr {
            Expr::MacroCall { name, path, delimiter: _, raw_tokens, span, .. } => {
                let expanded = self.expand_macro_call(name, &path, &raw_tokens, span);
                let replacement = self.arena.exprs[expanded.0 as usize].clone();
                self.arena.exprs[expr_id.0 as usize] = replacement;
                expr_id
            }
            Expr::Binary { op, left, right } => {
                let new_left = self.expand_expr(left);
                let new_right = self.expand_expr(right);
                self.arena.alloc_expr(Expr::Binary { op, left: new_left, right: new_right })
            }
            Expr::Unary { op, operand } => {
                let new_op = self.expand_expr(operand);
                self.arena.alloc_expr(Expr::Unary { op, operand: new_op })
            }
            Expr::Assign { lvalue, op, value } => {
                let new_l = self.expand_expr(lvalue);
                let new_v = self.expand_expr(value);
                self.arena.alloc_expr(Expr::Assign { lvalue: new_l, op, value: new_v })
            }
            Expr::Cast { expr, target_type } => {
                let new_expr = self.expand_expr(expr);
                let new_ty = self.expand_type(target_type);
                self.arena.alloc_expr(Expr::Cast { expr: new_expr, target_type: new_ty })
            }
            Expr::Identifier { segments, generic_args } => {
                let new_generic_args = generic_args.into_iter().map(|t| self.expand_type(t)).collect();
                self.arena.alloc_expr(Expr::Identifier { segments, generic_args: new_generic_args })
            }
            Expr::Call { callee, generic_args, args } => {
                let new_callee = self.expand_expr(callee);
                let new_generic_args = generic_args.into_iter().map(|t| self.expand_type(t)).collect();
                let mut new_args = Vec::new();
                for arg in args {
                    new_args.push(mellis_ast::CallArg {
                        label: arg.label,
                        value: self.expand_expr(arg.value),
                    });
                }
                self.arena.alloc_expr(Expr::Call { callee: new_callee, generic_args: new_generic_args, args: new_args })
            }
            Expr::MethodCall { object, method_name, generic_args, args } => {
                let new_obj = self.expand_expr(object);
                let new_generic_args = generic_args.into_iter().map(|t| self.expand_type(t)).collect();
                let mut new_args = Vec::new();
                for arg in args {
                    new_args.push(mellis_ast::CallArg {
                        label: arg.label,
                        value: self.expand_expr(arg.value),
                    });
                }
                self.arena.alloc_expr(Expr::MethodCall { object: new_obj, method_name, generic_args: new_generic_args, args: new_args })
            }
            Expr::Member { object, member } => {
                let new_obj = self.expand_expr(object);
                self.arena.alloc_expr(Expr::Member { object: new_obj, member })
            }
            Expr::Index { base, index } => {
                let new_base = self.expand_expr(base);
                let new_idx = self.expand_expr(index);
                self.arena.alloc_expr(Expr::Index { base: new_base, index: new_idx })
            }
            Expr::ArrayLiteral { elements } => {
                let new_elems = elements.into_iter().map(|e| self.expand_expr(e)).collect();
                self.arena.alloc_expr(Expr::ArrayLiteral { elements: new_elems })
            }
            Expr::TupleLiteral { elements } => {
                let new_elems = elements.into_iter().map(|e| self.expand_expr(e)).collect();
                self.arena.alloc_expr(Expr::TupleLiteral { elements: new_elems })
            }
            Expr::StructInit { path, generic_args, fields } => {
                let new_generic_args = generic_args.into_iter().map(|t| self.expand_type(t)).collect();
                let new_fields = fields.into_iter().map(|f| mellis_ast::FieldInit {
                    name: f.name,
                    value: self.expand_expr(f.value),
                }).collect();
                self.arena.alloc_expr(Expr::StructInit { path, generic_args: new_generic_args, fields: new_fields })
            }
            Expr::Lambda { params, return_type, body, is_move } => {
                let new_ret = return_type.map(|rt| self.expand_type(rt));
                let _ = self.expand_stmt(body);
                self.arena.alloc_expr(Expr::Lambda { params, return_type: new_ret, body, is_move })
            }
            Expr::Match { match_span, subject, arms } => {
                let new_subject = self.expand_expr(subject);
                let mut new_arms = Vec::new();
                for arm in arms {
                    let _ = self.expand_stmt(arm.body);
                    new_arms.push(arm);
                }
                self.arena.alloc_expr(Expr::Match { match_span, subject: new_subject, arms: new_arms })
            }
            _ => expr_id,
        }
    }

    pub fn expand_type(&mut self, ty_id: TypeId) -> TypeId {
        let ty_data = self.arena.types[ty_id.0 as usize].clone();
        match ty_data {
            Type::MacroCall { name, path, raw_tokens, span, .. } => {
                let path_strs: Vec<&str> = if !path.is_empty() {
                    path.iter().map(|seg| self.get_span_text(*seg)).collect()
                } else {
                    vec![self.get_span_text(name)]
                };
                let macro_name = path_strs.join("::");

                let Some(macro_sym) = self.symbol_table.lookup_macro(&path_strs, self.current_scope) else {
                    self.diagnostics.push(
                        Diagnostic::error(format!("no macro named `{}` in scope", macro_name))
                            .with_span(span),
                    );
                    return ty_id;
                };

                if !self.symbol_table.is_accessible(macro_sym, self.current_scope, None) {
                    self.diagnostics.push(
                        Diagnostic::error(format!("Macro `{}` is private and cannot be accessed from this scope", macro_name))
                            .with_span(span),
                    );
                    return ty_id;
                }

                let Some(&decl_id) = self.tables.macro_decls.get(&macro_sym) else {
                    return ty_id;
                };

                let rules = if let Decl::Macro { rules, .. } = &self.arena.decls[decl_id.0 as usize] {
                    rules.clone()
                } else {
                    Vec::new()
                };

                if self.recursion_depth > 128 {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "recursion limit reached while expanding macro `{}`",
                            macro_name
                        ))
                        .with_span(span),
                    );
                    return ty_id;
                }

                self.recursion_depth += 1;
                self.expansion_counter += 1;
                let expansion_id = self.expansion_counter;

                let mut best_failure: Option<MatchFailure> = None;
                for rule in &rules {
                    match self.match_rule(&raw_tokens, rule) {
                        Ok(captures) => {
                            let transcribed = self.transcribe_rule(rule, &captures, span, expansion_id);
                            let mut parser = Parser::from_tokens(transcribed, &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), self.arena, self.file_id);
                            match parser.parse_type() {
                                Ok(parsed_ty_id) => {
                                    self.diagnostics.extend(parser.diagnostics.clone());
                                    let final_ty = self.expand_type(parsed_ty_id);
                                    self.recursion_depth -= 1;
                                    let replacement = self.arena.types[final_ty.0 as usize].clone();
                                    self.arena.types[ty_id.0 as usize] = replacement;
                                    return ty_id;
                                }
                                Err(_) => {
                                    self.diagnostics.extend(parser.diagnostics.clone());
                                }
                            }
                        }
                        Err(failure) => {
                            if best_failure.as_ref().map_or(true, |b| failure.pos >= b.pos) {
                                best_failure = Some(failure);
                            }
                        }
                    }
                }

                self.recursion_depth -= 1;
                if let Some(failure) = best_failure {
                    let mut diag_span = span;
                    if failure.pos < raw_tokens.len() {
                        diag_span = raw_tokens[failure.pos].span;
                    }
                    self.diagnostics.push(
                        Diagnostic::error(format!("macro `{}` expected {}", macro_name, failure.expected))
                            .with_span(diag_span)
                    );
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "no rule in macro `{}` matched the invocation arguments",
                            macro_name
                        ))
                        .with_span(span),
                    );
                }
                ty_id
            }
            Type::Reference { is_mutable, lifetime, inner } => {
                let new_inner = self.expand_type(inner);
                let new_lifetime = lifetime.map(|lt| self.expand_type(lt));
                self.arena.alloc_type(Type::Reference { is_mutable, lifetime: new_lifetime, inner: new_inner })
            }
            Type::Pointer { is_mutable, inner } => {
                let new_inner = self.expand_type(inner);
                self.arena.alloc_type(Type::Pointer { is_mutable, inner: new_inner })
            }
            Type::Array { element_type, size } => {
                let new_elem = self.expand_type(element_type);
                let new_size = self.expand_expr(size);
                self.arena.alloc_type(Type::Array { element_type: new_elem, size: new_size })
            }
            Type::Slice { inner } => {
                let new_inner = self.expand_type(inner);
                self.arena.alloc_type(Type::Slice { inner: new_inner })
            }
            Type::Tuple { elements } => {
                let new_elems: Vec<TypeId> = elements.into_iter().map(|e| self.expand_type(e)).collect();
                self.arena.alloc_type(Type::Tuple { elements: new_elems })
            }
            Type::Function { params, return_type, is_unsafe } => {
                let new_params: Vec<TypeId> = params.into_iter().map(|p| self.expand_type(p)).collect();
                let new_ret = return_type.map(|r| self.expand_type(r));
                self.arena.alloc_type(Type::Function { params: new_params, return_type: new_ret, is_unsafe })
            }
            Type::Named { segments, generic_args, associated_bindings } => {
                let new_args: Vec<TypeId> = generic_args.into_iter().map(|a| self.expand_type(a)).collect();
                let new_bindings: Vec<mellis_ast::AssociatedBinding> = associated_bindings.into_iter().map(|b| {
                    mellis_ast::AssociatedBinding {
                        name: b.name,
                        ty: self.expand_type(b.ty),
                    }
                }).collect();
                self.arena.alloc_type(Type::Named { segments, generic_args: new_args, associated_bindings: new_bindings })
            }
            _ => ty_id,
        }
    }

    fn expand_macro_call(&mut self, name: Span, path: &[Span], raw_tokens: &[Token], call_span: Span) -> ExprId {
        let path_strs: Vec<&str> = if !path.is_empty() {
            path.iter().map(|seg| self.get_span_text(*seg)).collect()
        } else {
            vec![self.get_span_text(name)]
        };
        let macro_name = path_strs.join("::");

        let Some(macro_sym) = self.symbol_table.lookup_macro(&path_strs, self.current_scope) else {
            self.diagnostics.push(
                Diagnostic::error(format!("no macro named `{}` in scope", macro_name))
                    .with_span(if !path.is_empty() {
                        Span::new(
                            path[0].file_id,
                            path[0].start,
                            path.last().unwrap().end,
                        ).with_ctxt(path[0].ctxt)
                    } else {
                        name
                    }),
            );
            return self.arena.alloc_expr(Expr::Literal(Token::new(TokenKind::IntegerLiteral, call_span), "0".to_string()));
        };

        if !self.symbol_table.is_accessible(macro_sym, self.current_scope, None) {
            self.diagnostics.push(
                Diagnostic::error(format!("Macro `{}` is private and cannot be accessed from this scope", macro_name))
                    .with_span(call_span),
            );
            return self.arena.alloc_expr(Expr::Literal(Token::new(TokenKind::IntegerLiteral, call_span), "0".to_string()));
        }

        let Some(&decl_id) = self.tables.macro_decls.get(&macro_sym) else {
            self.diagnostics.push(
                Diagnostic::error(format!("no macro declaration found for `{}`", macro_name))
                    .with_span(name),
            );
            return self.arena.alloc_expr(Expr::Literal(Token::new(TokenKind::IntegerLiteral, call_span), "0".to_string()));
        };

        let rules = if let Decl::Macro { rules, .. } = &self.arena.decls[decl_id.0 as usize] {
            rules.clone()
        } else {
            Vec::new()
        };

        if self.recursion_depth > 128 {
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "recursion limit reached while expanding macro `{}`",
                    macro_name
                ))
                .with_span(call_span),
            );
            return self.arena.alloc_expr(Expr::Literal(Token::new(TokenKind::IntegerLiteral, call_span), "0".to_string()));
        }

        self.recursion_depth += 1;
        self.expansion_counter += 1;
        let expansion_id = self.expansion_counter;

        // Try rules in order
        let mut best_failure: Option<MatchFailure> = None;
        for rule in &rules {
            match self.match_rule(raw_tokens, rule) {
                Ok(captures) => {
                    let transcribed = self.transcribe_rule(rule, &captures, call_span, expansion_id);
                    
                    // Parse transcribed tokens as expression
                    let mut parser = Parser::from_tokens(transcribed, &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), self.arena, self.file_id);
                    match parser.parse_expression(true) {
                        Ok(parsed_expr) => {
                            self.diagnostics.extend(parser.diagnostics);
                            // Recursively expand if the parsed expression contains macro calls
                            let final_expr = self.expand_expr(parsed_expr);
                            self.recursion_depth -= 1;
                            return final_expr;
                        }
                        Err(()) => {
                            self.diagnostics.extend(parser.diagnostics);
                            self.recursion_depth -= 1;
                            return self.arena.alloc_expr(Expr::Literal(Token::new(TokenKind::IntegerLiteral, call_span), "0".to_string()));
                        }
                    }
                }
                Err(failure) => {
                    if best_failure.as_ref().map_or(true, |b| failure.pos >= b.pos) {
                        best_failure = Some(failure);
                    }
                }
            }
        }

        self.recursion_depth -= 1;
        if let Some(failure) = best_failure {
            let mut diag_span = call_span;
            if failure.pos < raw_tokens.len() {
                diag_span = raw_tokens[failure.pos].span;
            }
            self.diagnostics.push(
                Diagnostic::error(format!("macro `{}` expected {}", macro_name, failure.expected))
                    .with_span(diag_span)
            );
        } else {
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "no rule in macro `{}` matched the invocation arguments",
                    macro_name
                ))
                .with_span(call_span),
            );
        }
        self.arena.alloc_expr(Expr::Literal(Token::new(TokenKind::IntegerLiteral, call_span), "0".to_string()))
    }

    fn expand_macro_call_items(
        &mut self,
        name: Span,
        path: &[Span],
        raw_tokens: &[Token],
        call_span: Span,
        has_semi: bool,
    ) -> Option<Vec<Item>> {
        let path_strs: Vec<&str> = if !path.is_empty() {
            path.iter().map(|seg| self.get_span_text(*seg)).collect()
        } else {
            vec![self.get_span_text(name)]
        };
        let macro_name = path_strs.join("::");

        let macro_sym = self.symbol_table.lookup_macro(&path_strs, self.current_scope)?;
        if !self.symbol_table.is_accessible(macro_sym, self.current_scope, None) {
            self.diagnostics.push(
                Diagnostic::error(format!("Macro `{}` is private and cannot be accessed from this scope", macro_name))
                    .with_span(call_span),
            );
            return None;
        }
        let &decl_id = self.tables.macro_decls.get(&macro_sym)?;
        let rules = if let Decl::Macro { rules, .. } = &self.arena.decls[decl_id.0 as usize] {
            rules.clone()
        } else {
            Vec::new()
        };

        if self.recursion_depth > 128 {
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "recursion limit reached while expanding macro `{}`",
                    macro_name
                ))
                .with_span(call_span),
            );
            return None;
        }

        self.recursion_depth += 1;
        self.expansion_counter += 1;
        let expansion_id = self.expansion_counter;

        let mut best_failure: Option<MatchFailure> = None;
        let has_trace = self.tables.macro_decls.get(&macro_sym).and_then(|id| {
            let decl = &self.arena.decls[id.0 as usize];
            if let Decl::Macro { annotations, .. } = decl {
                Some(annotations.iter().any(|a| self.get_span_text(a.name) == "macro_trace"))
            } else {
                None
            }
        }).unwrap_or(false);

        if has_trace {

        }

        for rule in &rules {
            match self.match_rule(raw_tokens, rule) {
                Ok(captures) => {
                let transcribed = self.transcribe_rule(rule, &captures, call_span, expansion_id);

                // Try parsing items (declarations and statements)
                let mut parser = Parser::from_tokens(transcribed.clone(), &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), self.arena, self.file_id);
                let mut parsed_items = Vec::new();
                let mut success = true;
                while !parser.is_at_end() {
                    match parser.parse_item() {
                        Ok(it) => parsed_items.push(it),
                        Err(_) => {
                            success = false;
                            break;
                        }
                    }
                }

                if success && !parsed_items.is_empty() && parser.diagnostics.is_empty() {
                    let mut expanded = Vec::new();
                    for it in parsed_items {
                        expanded.extend(self.expand_block_item(it));
                    }
                    self.recursion_depth -= 1;
                    return Some(expanded);
                }

                // Fallback: try parsing as expression and wrapping in Stmt::Expr
                let mut parser = Parser::from_tokens(transcribed, &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), self.arena, self.file_id);
                if let Ok(expr_id) = parser.parse_expression(true) {
                    self.diagnostics.extend(parser.diagnostics);
                    let final_expr = self.expand_expr(expr_id);
                    let stmt_id = self.arena.alloc_stmt(Stmt::Expr { expr: final_expr, has_semicolon: has_semi });
                    self.recursion_depth -= 1;
                    return Some(vec![Item::Stmt(stmt_id)]);
                }
                }
                Err(failure) => {
                    if best_failure.as_ref().map_or(true, |b| failure.pos >= b.pos) {
                        best_failure = Some(failure);
                    }
                }
            }
        }

        self.recursion_depth -= 1;
        if let Some(failure) = best_failure {
            let mut diag_span = call_span;
            if failure.pos < raw_tokens.len() {
                diag_span = raw_tokens[failure.pos].span;
            }
            self.diagnostics.push(
                Diagnostic::error(format!("macro `{}` expected {}", macro_name, failure.expected))
                    .with_span(diag_span)
            );
        }
        None
    }

    fn match_rule(
        &mut self,
        tokens: &[Token],
        rule: &MacroRule,
    ) -> Result<HashMap<String, CapturedFragment>, MatchFailure> {
        let mut captures = HashMap::new();
        let consumed = self.match_pattern_elements(tokens, 0, &rule.pattern.elements, &mut captures)?;
        if consumed == tokens.len() {
            return Ok(captures);
        }
        Err(MatchFailure { pos: consumed, expected: "end of macro invocation".to_string(), span: rule.span })
    }

    fn match_pattern_elements(
        &self,
        tokens: &[Token],
        mut pos: usize,
        elements: &[MatcherElement],
        captures: &mut HashMap<String, CapturedFragment>,
    ) -> Result<usize, MatchFailure> {
        for element in elements {
            match element {
                MatcherElement::Leaf { token: pat_tok } => {
                    if pos >= tokens.len() {
                        return Err(MatchFailure { pos, expected: format!("{:?}", pat_tok.kind), span: pat_tok.span });
                    }
                    if tokens[pos].kind != pat_tok.kind {
                        return Err(MatchFailure { pos, expected: format!("{:?}", pat_tok.kind), span: tokens[pos].span });
                    }
                    pos += 1;
                }
                MatcherElement::Group { delimiter, elements: group_elements, span } => {
                    if pos >= tokens.len() {
                        return Err(MatchFailure { pos, expected: "group".to_string(), span: *span });
                    }
                    let (open_kind, close_kind) = match delimiter {
                        MacroDelimiter::Paren => (TokenKind::LParen, TokenKind::RParen),
                        MacroDelimiter::Bracket => (TokenKind::LBracket, TokenKind::RBracket),
                        MacroDelimiter::Brace => (TokenKind::LBrace, TokenKind::RBrace),
                    };

                    if tokens[pos].kind != open_kind {
                        return Err(MatchFailure { pos, expected: format!("{:?}", open_kind), span: tokens[pos].span });
                    }
                    pos += 1;

                    let group_start = pos;
                    let mut depth = 1;
                    while pos < tokens.len() {
                        if tokens[pos].kind == open_kind {
                            depth += 1;
                        } else if tokens[pos].kind == close_kind {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        pos += 1;
                    }

                    if depth != 0 || pos >= tokens.len() {
                        return Err(MatchFailure { pos, expected: format!("{:?}", close_kind), span: *span });
                    }

                    let inner_tokens = &tokens[group_start..pos];
                    pos += 1; // consume close_kind

                    let inner_consumed = self.match_pattern_elements(inner_tokens, 0, group_elements, captures)?;
                    if inner_consumed != inner_tokens.len() {
                        return Err(MatchFailure { pos, expected: "end of group".to_string(), span: *span });
                    }
                }
                MatcherElement::MetaVar { name, fragment, .. } => {
                    if pos >= tokens.len() {
                        return Err(MatchFailure { pos, expected: format!("{:?}", fragment), span: *name });
                    }
                    let var_name = self.get_span_text(*name).to_string();
                    let consumed = self.capture_fragment_parser(tokens, pos, *fragment).map_err(|e| MatchFailure { pos, expected: e, span: *name })?;
                    if consumed == 0 {
                        return Err(MatchFailure { pos, expected: format!("{:?}", fragment), span: *name });
                    }
                    let captured_tokens = tokens[pos..pos + consumed].to_vec();
                    captures.insert(var_name, CapturedFragment::Single(captured_tokens));
                    pos += consumed;
                }
                MatcherElement::Repetition { elements: rep_elements, separator, kind, span } => {
                    let mut repeated_captures: HashMap<String, Vec<CapturedFragment>> = HashMap::new();
                    let mut repetition_count = 0;
                    
                    while pos < tokens.len() {
                        let mut temp_captures = HashMap::new();
                        let start_pos = pos;
                        let res = self.match_pattern_elements(tokens, pos, rep_elements, &mut temp_captures);
                        match res {
                            Ok(new_pos) => {
                                if new_pos == start_pos {
                                    break;
                                }
                                pos = new_pos;
                                repetition_count += 1;
                                
                                for (k, v) in temp_captures {
                                    repeated_captures.entry(k).or_insert_with(Vec::new).push(v);
                                }
                                
                                if let Some(sep) = separator {
                                    if pos < tokens.len() && tokens[pos].kind == *sep {
                                        pos += 1;
                                    } else {
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                    
                    if *kind == mellis_ast::RepetitionKind::OneOrMore && repetition_count == 0 {
                        return Err(MatchFailure { pos, expected: "at least one repetition".to_string(), span: *span });
                    }
                    
                    // We must ensure that any variable declared inside the repetition has a list, 
                    // even if it was not captured in some iterations. 
                    // For simplicity, we just insert the collected captures as CapturedFragment::Repeated.
                    // A proper implementation would pad missing variables, but this handles standard cases.
                    for (k, v) in repeated_captures {
                        // If it already exists in captures, it means we have a collision.
                        // But since it's nested, we just insert it.
                        captures.insert(k, CapturedFragment::Repeated(v));
                    }
                }
            }
        }
        Ok(pos)
    }

    fn capture_fragment_parser(
        &self,
        tokens: &[Token],
        pos: usize,
        fragment: FragmentKind,
    ) -> Result<usize, String> {
        if pos >= tokens.len() {
            return Err("unexpected end of input".to_string());
        }

        match fragment {
            FragmentKind::Ident => {
                let tok = tokens[pos];
                if tok.kind == TokenKind::Identifier || tok.kind == TokenKind::KwSelfVal {
                    Ok(1)
                } else {
                    Err(format!("expected {}", match fragment { FragmentKind::Lifetime => "lifetime", _ => "identifier" }))
                }
            }
            FragmentKind::Expr | FragmentKind::Ty | FragmentKind::Tt | FragmentKind::Pat | FragmentKind::Path | FragmentKind::Meta => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), &mut scratch_arena, self.file_id);
                
                let success = match fragment {
                    FragmentKind::Expr => parser.parse_expression(true).is_ok(),
                    FragmentKind::Ty => parser.parse_type().is_ok(),
                    FragmentKind::Stmt => parser.parse_stmt().is_ok(),
                    FragmentKind::Block => parser.parse_block_stmt().is_ok(),
                    FragmentKind::Item => parser.parse_item().is_ok(),
                    FragmentKind::Tt => parser.parse_token_tree().is_ok(),
                    FragmentKind::Pat => parser.parse_pattern().is_ok(),
                    FragmentKind::Path => parser.parse_value_path().is_ok(),
                    FragmentKind::Meta => parser.parse_annotations().is_ok(),
                    _ => unreachable!(),
                };

                if success && parser.pos() > 0 {
                    Ok(parser.pos())
                } else {
                    Err(format!("expected {:?}", fragment))
                }
            }
            FragmentKind::Literal => {
                let tok = tokens[pos];
                if matches!(
                    tok.kind,
                    TokenKind::IntegerLiteral
                        | TokenKind::FloatLiteral
                        | TokenKind::StringLiteral
                        | TokenKind::CharLiteral
                        | TokenKind::ByteLiteral
                        | TokenKind::ByteStringLiteral
                        | TokenKind::RawStringLiteral
                        | TokenKind::KwTrue
                        | TokenKind::KwFalse
                ) {
                    Ok(1)
                } else {
                    Err(format!("expected {:?}", fragment))
                }
            }
            FragmentKind::Vis => {
                let tok = tokens[pos];
                if tok.kind == TokenKind::KwPub {
                    // Check for `pub(crate)` or `pub(super)`
                    if pos + 3 < tokens.len() 
                        && tokens[pos + 1].kind == TokenKind::LParen 
                        && matches!(tokens[pos + 2].kind, TokenKind::KwCrate | TokenKind::KwSuper)
                        && tokens[pos + 3].kind == TokenKind::RParen 
                    {
                        Ok(4)
                    } else {
                        Ok(1)
                    }
                } else {
                    Ok(0) // Empty visibility (private) is valid
                }
            }
            FragmentKind::Lifetime => {
                let tok = tokens[pos];
                if tok.kind == TokenKind::Lifetime {
                    Ok(1)
                } else {
                    Err("expected lifetime".to_string())
                }
            }
            FragmentKind::Stmt => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), &mut scratch_arena, self.file_id);
                match parser.parse_stmt() {
                    Ok(_) if parser.pos() > 0 => Ok(parser.pos()),
                    _ => Err(format!("expected {:?}", fragment)),
                }
            }
            FragmentKind::Block => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), &mut scratch_arena, self.file_id);
                match parser.parse_block_stmt() {
                    Ok(_) if parser.pos() > 0 => Ok(parser.pos()),
                    _ => Err(format!("expected {:?}", fragment)),
                }
            }
            FragmentKind::Item => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, &self.source_manager.get_file(self.file_id).unwrap().source, Some(self.source_manager), &mut scratch_arena, self.file_id);
                match parser.parse_item() {
                    Ok(_) if parser.pos() > 0 => Ok(parser.pos()),
                    _ => Err(format!("expected {:?}", fragment)),
                }
            }
        }
    }

    fn transcribe_rule(
        &mut self,
        rule: &MacroRule,
        captures: &HashMap<String, CapturedFragment>,
        call_span: Span,
        expansion_id: u32,
    ) -> Vec<Token> {
        let mut output = Vec::new();
        self.transcribe_elements(&rule.transcriber.elements, captures, call_span, expansion_id, &mut output);
        output
    }

    fn transcribe_elements(
        &mut self,
        elements: &[TranscriberElement],
        captures: &HashMap<String, CapturedFragment>,
        call_span: Span,
        expansion_id: u32,
        output: &mut Vec<Token>,
    ) {
        for element in elements {
            match element {
                TranscriberElement::Leaf { token } => {
                    let mut tok = *token;
                    tok.span.ctxt = SyntaxContext(expansion_id);
                    output.push(tok);
                }
                TranscriberElement::Group { delimiter, elements: group_elements, span } => {
                    let (open_kind, close_kind) = match delimiter {
                        MacroDelimiter::Paren => (TokenKind::LParen, TokenKind::RParen),
                        MacroDelimiter::Bracket => (TokenKind::LBracket, TokenKind::RBracket),
                        MacroDelimiter::Brace => (TokenKind::LBrace, TokenKind::RBrace),
                    };
                    let mut open_span = *span;
                    open_span.ctxt = SyntaxContext(expansion_id);
                    let mut close_span = *span;
                    close_span.ctxt = SyntaxContext(expansion_id);

                    output.push(Token::new(open_kind, open_span));
                    self.transcribe_elements(group_elements, captures, call_span, expansion_id, output);
                    output.push(Token::new(close_kind, close_span));
                }
                TranscriberElement::MetaVar { name, .. } => {
                    let var_name = self.get_span_text(*name);
                    if let Some(captured) = captures.get(var_name) {
                        self.push_capture(captured, output, call_span);
                    } else {
                        self.diagnostics.push(
                            Diagnostic::error(format!("variable `@{}` is not bound in pattern", var_name))
                                .with_span(*name)
                        );
                    }
                }
                TranscriberElement::Repetition { elements: rep_elements, separator, kind: _, span: _ } => {
                    // Find the repetition length by inspecting the bound variables in the inner elements
                    let mut max_len = 0;
                    self.find_repetition_length(rep_elements, captures, &mut max_len);

                    for idx in 0..max_len {
                        if idx > 0 {
                            if let Some(sep) = separator {
                                output.push(Token::new(*sep, call_span));
                            }
                        }
                        
                        let mut sub_captures = captures.clone();
                        self.extract_repetition_idx(rep_elements, captures, idx, &mut sub_captures);
                        
                        self.transcribe_elements(rep_elements, &sub_captures, call_span, expansion_id, output);
                    }
                }
            }
        }
    }

    fn push_capture(&self, captured: &CapturedFragment, output: &mut Vec<Token>, call_span: Span) {
        match captured {
            CapturedFragment::Single(tokens) => {
                output.extend(tokens.clone());
            }
            CapturedFragment::Repeated(list) => {
                for (idx, item) in list.iter().enumerate() {
                    if idx > 0 {
                        output.push(Token::new(TokenKind::Comma, call_span));
                    }
                    self.push_capture(item, output, call_span);
                }
            }
        }
    }

    fn find_repetition_length(&self, elements: &[TranscriberElement], captures: &HashMap<String, CapturedFragment>, max_len: &mut usize) {
        for element in elements {
            match element {
                TranscriberElement::MetaVar { name, .. } => {
                    let var_name = self.get_span_text(*name);
                    if let Some(CapturedFragment::Repeated(list)) = captures.get(var_name) {
                        *max_len = (*max_len).max(list.len());
                    }
                }
                TranscriberElement::Group { elements: inner, .. } => {
                    self.find_repetition_length(inner, captures, max_len);
                }
                TranscriberElement::Repetition { elements: inner, .. } => {
                    // Nested repetition - we don't look inside because its length is determined by its own loop
                }
                _ => {}
            }
        }
    }

    fn extract_repetition_idx(&self, elements: &[TranscriberElement], captures: &HashMap<String, CapturedFragment>, idx: usize, sub_captures: &mut HashMap<String, CapturedFragment>) {
        for element in elements {
            match element {
                TranscriberElement::MetaVar { name, .. } => {
                    let var_name = self.get_span_text(*name);
                    if let Some(CapturedFragment::Repeated(list)) = captures.get(var_name) {
                        if idx < list.len() {
                            sub_captures.insert(var_name.to_string(), list[idx].clone());
                        }
                    }
                }
                TranscriberElement::Group { elements: inner, .. } => {
                    self.extract_repetition_idx(inner, captures, idx, sub_captures);
                }
                TranscriberElement::Repetition { elements: inner, .. } => {
                    // Do not descend, inner will extract its own indices
                }
                _ => {}
            }
        }
    }

}
