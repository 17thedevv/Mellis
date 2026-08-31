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
    Repeated(Vec<Vec<Token>>),
}

pub struct MacroEngine<'a> {
    pub arena: &'a mut AstArena,
    pub source: &'a str,
    pub file_id: FileId,
    pub symbol_table: &'a SymbolTable,
    pub tables: &'a SemanticTables,
    pub current_scope: ScopeId,
    pub diagnostics: Vec<Diagnostic>,
    expansion_counter: u32,
    recursion_depth: u32,
}

impl<'a> MacroEngine<'a> {
    pub fn new(
        arena: &'a mut AstArena,
        source: &'a str,
        file_id: FileId,
        symbol_table: &'a SymbolTable,
        tables: &'a SemanticTables,
    ) -> Self {
        Self {
            arena,
            source,
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
                let name_str = &self.source[name.start as usize..name.end as usize];
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
                let name_str = &self.source[name.start as usize..name.end as usize];
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
                    path.iter().map(|seg| &self.source[seg.start as usize..seg.end as usize]).collect()
                } else {
                    vec![&self.source[name.start as usize..name.end as usize]]
                };
                let macro_name = path_strs.join("::");

                let Some(macro_sym) = self.symbol_table.lookup_macro(&path_strs, self.current_scope) else {
                    self.diagnostics.push(
                        Diagnostic::error(format!("no macro named `{}` in scope", macro_name))
                            .with_span(span),
                    );
                    return ty_id;
                };

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

                for rule in &rules {
                    if let Some(captures) = self.match_rule(&raw_tokens, rule) {
                        let transcribed = self.transcribe_rule(rule, &captures, span, expansion_id);
                        let mut parser = Parser::from_tokens(transcribed, self.source, self.arena, self.file_id);
                        match parser.parse_type() {
                            Ok(parsed_ty_id) => {
                                self.diagnostics.extend(parser.diagnostics);
                                let final_ty = self.expand_type(parsed_ty_id);
                                self.recursion_depth -= 1;
                                let replacement = self.arena.types[final_ty.0 as usize].clone();
                                self.arena.types[ty_id.0 as usize] = replacement;
                                return ty_id;
                            }
                            Err(_) => {
                                self.diagnostics.extend(parser.diagnostics);
                            }
                        }
                    }
                }

                self.recursion_depth -= 1;
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "no rule in macro `{}` matched the invocation arguments",
                        macro_name
                    ))
                    .with_span(span),
                );
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
            path.iter().map(|seg| &self.source[seg.start as usize..seg.end as usize]).collect()
        } else {
            vec![&self.source[name.start as usize..name.end as usize]]
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
            return self.arena.alloc_expr(Expr::Literal(Token::new(
                TokenKind::IntegerLiteral,
                call_span,
            )));
        };

        let Some(&decl_id) = self.tables.macro_decls.get(&macro_sym) else {
            self.diagnostics.push(
                Diagnostic::error(format!("no macro declaration found for `{}`", macro_name))
                    .with_span(name),
            );
            return self.arena.alloc_expr(Expr::Literal(Token::new(
                TokenKind::IntegerLiteral,
                call_span,
            )));
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
            return self.arena.alloc_expr(Expr::Literal(Token::new(
                TokenKind::IntegerLiteral,
                call_span,
            )));
        }

        self.recursion_depth += 1;
        self.expansion_counter += 1;
        let expansion_id = self.expansion_counter;

        // Try rules in order
        for rule in &rules {
            if let Some(captures) = self.match_rule(raw_tokens, rule) {
                let transcribed = self.transcribe_rule(rule, &captures, call_span, expansion_id);
                
                // Parse transcribed tokens as expression
                let mut parser = Parser::from_tokens(transcribed, self.source, self.arena, self.file_id);
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
                        return self.arena.alloc_expr(Expr::Literal(Token::new(
                            TokenKind::IntegerLiteral,
                            call_span,
                        )));
                    }
                }
            }
        }

        self.recursion_depth -= 1;
        self.diagnostics.push(
            Diagnostic::error(format!(
                "no rule in macro `{}` matched the invocation arguments",
                macro_name
            ))
            .with_span(call_span),
        );
        self.arena.alloc_expr(Expr::Literal(Token::new(
            TokenKind::IntegerLiteral,
            call_span,
        )))
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
            path.iter().map(|seg| &self.source[seg.start as usize..seg.end as usize]).collect()
        } else {
            vec![&self.source[name.start as usize..name.end as usize]]
        };
        let macro_name = path_strs.join("::");

        let macro_sym = self.symbol_table.lookup_macro(&path_strs, self.current_scope)?;
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

        for rule in &rules {
            if let Some(captures) = self.match_rule(raw_tokens, rule) {
                let transcribed = self.transcribe_rule(rule, &captures, call_span, expansion_id);

                // Try parsing items (declarations and statements)
                let mut parser = Parser::from_tokens(transcribed.clone(), self.source, self.arena, self.file_id);
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
                let mut parser = Parser::from_tokens(transcribed, self.source, self.arena, self.file_id);
                if let Ok(expr_id) = parser.parse_expression(true) {
                    self.diagnostics.extend(parser.diagnostics);
                    let final_expr = self.expand_expr(expr_id);
                    let stmt_id = self.arena.alloc_stmt(Stmt::Expr { expr: final_expr, has_semicolon: has_semi });
                    self.recursion_depth -= 1;
                    return Some(vec![Item::Stmt(stmt_id)]);
                }
            }
        }

        self.recursion_depth -= 1;
        None
    }

    fn match_rule(
        &self,
        tokens: &[Token],
        rule: &MacroRule,
    ) -> Option<HashMap<String, CapturedFragment>> {
        // If structured pattern elements exist, use tree matching
        if !rule.pattern.elements.is_empty() {
            let mut captures = HashMap::new();
            if let Some(consumed) = self.match_pattern_elements(tokens, 0, &rule.pattern.elements, &mut captures) {
                if consumed == tokens.len() {
                    return Some(captures);
                }
            }
            return None;
        }

        // Fallback for flat matchers
        self.match_flat_rule(tokens, &rule.matchers)
    }

    fn match_pattern_elements(
        &self,
        tokens: &[Token],
        mut pos: usize,
        elements: &[MatcherElement],
        captures: &mut HashMap<String, CapturedFragment>,
    ) -> Option<usize> {
        for element in elements {
            match element {
                MatcherElement::Leaf { token: pat_tok } => {
                    if pos >= tokens.len() {
                        return None;
                    }
                    if tokens[pos].kind != pat_tok.kind {
                        return None;
                    }
                    pos += 1;
                }
                MatcherElement::Group { delimiter, elements: group_elements, .. } => {
                    if pos >= tokens.len() {
                        return None;
                    }
                    let (open_kind, close_kind) = match delimiter {
                        MacroDelimiter::Paren => (TokenKind::LParen, TokenKind::RParen),
                        MacroDelimiter::Bracket => (TokenKind::LBracket, TokenKind::RBracket),
                        MacroDelimiter::Brace => (TokenKind::LBrace, TokenKind::RBrace),
                    };

                    if tokens[pos].kind != open_kind {
                        return None;
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
                        return None;
                    }

                    let inner_tokens = &tokens[group_start..pos];
                    pos += 1; // consume close_kind

                    if let Some(inner_consumed) = self.match_pattern_elements(inner_tokens, 0, group_elements, captures) {
                        if inner_consumed != inner_tokens.len() {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                MatcherElement::MetaVar { name, fragment, .. } => {
                    if pos >= tokens.len() {
                        return None;
                    }
                    let var_name = self.source[name.start as usize..name.end as usize].to_string();
                    let consumed = self.capture_fragment_parser(tokens, pos, *fragment)?;
                    if consumed == 0 {
                        return None;
                    }
                    let captured_tokens = tokens[pos..pos + consumed].to_vec();
                    captures.insert(var_name, CapturedFragment::Single(captured_tokens));
                    pos += consumed;
                }
                MatcherElement::Repetition { .. } => {
                    return None;
                }
            }
        }
        Some(pos)
    }

    fn capture_fragment_parser(
        &self,
        tokens: &[Token],
        pos: usize,
        fragment: FragmentKind,
    ) -> Option<usize> {
        if pos >= tokens.len() {
            return None;
        }

        match fragment {
            FragmentKind::Ident => {
                let tok = tokens[pos];
                if tok.kind == TokenKind::Identifier || tok.kind == TokenKind::KwSelfVal {
                    Some(1)
                } else {
                    None
                }
            }
            FragmentKind::Expr => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_expression(true) {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Ty => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_type() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Stmt => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_stmt() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Block => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_block_stmt() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
            FragmentKind::Item => {
                let remaining = tokens[pos..].to_vec();
                let mut scratch_arena = AstArena::new();
                let mut parser = Parser::from_tokens(remaining, self.source, &mut scratch_arena, self.file_id);
                match parser.parse_item() {
                    Ok(_) if parser.pos() > 0 => Some(parser.pos()),
                    _ => None,
                }
            }
        }
    }

    fn transcribe_rule(
        &self,
        rule: &MacroRule,
        captures: &HashMap<String, CapturedFragment>,
        call_span: Span,
        expansion_id: u32,
    ) -> Vec<Token> {
        if !rule.transcriber.elements.is_empty() {
            let mut output = Vec::new();
            self.transcribe_elements(&rule.transcriber.elements, captures, call_span, expansion_id, &mut output);
            return output;
        }

        // Fallback for flat template tokens
        self.transcribe_flat_template(&rule.template_tokens, captures, call_span, expansion_id)
    }

    fn transcribe_elements(
        &self,
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
                    let var_name = &self.source[name.start as usize..name.end as usize];
                    if let Some(captured) = captures.get(var_name) {
                        match captured {
                            CapturedFragment::Single(tokens) => {
                                output.extend(tokens.clone());
                            }
                            CapturedFragment::Repeated(list) => {
                                for (idx, item) in list.iter().enumerate() {
                                    if idx > 0 {
                                        output.push(Token::new(TokenKind::Comma, call_span));
                                    }
                                    output.extend(item.clone());
                                }
                            }
                        }
                    }
                }
                TranscriberElement::Repetition { .. } => {}
            }
        }
    }

    fn match_flat_rule(
        &self,
        tokens: &[Token],
        matchers: &[MacroMatcher],
    ) -> Option<HashMap<String, CapturedFragment>> {
        let mut captures = HashMap::new();
        let mut pos = 0;

        for (idx, matcher) in matchers.iter().enumerate() {
            let var_name = self.source[matcher.name.start as usize..matcher.name.end as usize].to_string();
            let is_last = idx == matchers.len() - 1;

            if let Some(rep_kind) = matcher.repetition {
                let mut repeated_tokens = Vec::new();
                let sep = matcher.separator;

                while pos < tokens.len() {
                    let frag_tokens = self.capture_one_fragment(tokens, &mut pos, matcher.fragment, sep, is_last)?;
                    repeated_tokens.push(frag_tokens);

                    if pos < tokens.len() {
                        if let Some(s) = sep {
                            if tokens[pos].kind == s {
                                pos += 1;
                                continue;
                            } else {
                                break;
                            }
                        } else if tokens[pos].kind == TokenKind::Comma {
                            pos += 1;
                            continue;
                        } else {
                            break;
                        }
                    }
                }

                if rep_kind == RepetitionKind::OneOrMore && repeated_tokens.is_empty() {
                    return None;
                }

                captures.insert(var_name, CapturedFragment::Repeated(repeated_tokens));
            } else {
                let frag_tokens = self.capture_one_fragment(tokens, &mut pos, matcher.fragment, Some(TokenKind::Comma), is_last)?;
                captures.insert(var_name, CapturedFragment::Single(frag_tokens));

                if !is_last {
                    if pos < tokens.len() && tokens[pos].kind == TokenKind::Comma {
                        pos += 1;
                    } else {
                        return None;
                    }
                }
            }
        }

        if pos == tokens.len() {
            Some(captures)
        } else {
            None
        }
    }

    fn capture_one_fragment(
        &self,
        tokens: &[Token],
        pos: &mut usize,
        fragment: FragmentKind,
        stop_sep: Option<TokenKind>,
        _is_last: bool,
    ) -> Option<Vec<Token>> {
        if *pos >= tokens.len() {
            return None;
        }

        match fragment {
            FragmentKind::Ident => {
                if tokens[*pos].kind == TokenKind::Identifier || tokens[*pos].kind == TokenKind::KwSelfVal {
                    let tok = tokens[*pos];
                    *pos += 1;
                    Some(vec![tok])
                } else {
                    None
                }
            }
            FragmentKind::Expr | FragmentKind::Ty | FragmentKind::Stmt | FragmentKind::Block | FragmentKind::Item => {
                let mut depth_paren = 0;
                let mut depth_bracket = 0;
                let mut depth_brace = 0;
                let start = *pos;

                while *pos < tokens.len() {
                    let tok = tokens[*pos];
                    match tok.kind {
                        TokenKind::LParen => depth_paren += 1,
                        TokenKind::RParen => {
                            if depth_paren == 0 { break; }
                            depth_paren -= 1;
                        }
                        TokenKind::LBracket => depth_bracket += 1,
                        TokenKind::RBracket => {
                            if depth_bracket == 0 { break; }
                            depth_bracket -= 1;
                        }
                        TokenKind::LBrace => depth_brace += 1,
                        TokenKind::RBrace => {
                            if depth_brace == 0 { break; }
                            depth_brace -= 1;
                        }
                        k => {
                            if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 {
                                if Some(k) == stop_sep || k == TokenKind::Comma {
                                    break;
                                }
                            }
                        }
                    }
                    *pos += 1;
                }

                if *pos > start {
                    Some(tokens[start..*pos].to_vec())
                } else {
                    None
                }
            }
        }
    }

    fn transcribe_flat_template(
        &self,
        template: &[Token],
        captures: &HashMap<String, CapturedFragment>,
        call_span: Span,
        expansion_id: u32,
    ) -> Vec<Token> {
        let mut output = Vec::new();
        let mut i = 0;

        while i < template.len() {
            let tok = template[i];
            
            // Check for `$(` or `@(` repetition block in template
            if (tok.kind == TokenKind::Dollar || tok.kind == TokenKind::At)
                && i + 1 < template.len()
                && template[i + 1].kind == TokenKind::LParen
            {
                i += 2; // skip `$` and `(`
                let start = i;
                let mut depth = 1;
                while i < template.len() {
                    if template[i].kind == TokenKind::LParen {
                        depth += 1;
                    } else if template[i].kind == TokenKind::RParen {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    i += 1;
                }
                let rep_template = &template[start..i];
                if i < template.len() {
                    i += 1; // consume `)`
                }

                let mut rep_sep = None;
                if i < template.len() {
                    if template[i].kind == TokenKind::Multiply || template[i].kind == TokenKind::Plus {
                        i += 1;
                    } else if i + 1 < template.len() && (template[i + 1].kind == TokenKind::Multiply || template[i + 1].kind == TokenKind::Plus) {
                        rep_sep = Some(template[i]);
                        i += 2;
                    }
                }

                let mut max_len = 0;
                for t in rep_template {
                    if t.kind == TokenKind::Identifier {
                        let name = &self.source[t.span.start as usize..t.span.end as usize];
                        if let Some(CapturedFragment::Repeated(list)) = captures.get(name) {
                            max_len = max_len.max(list.len());
                        }
                    }
                }

                for idx in 0..max_len {
                    if idx > 0 {
                        if let Some(sep_tok) = rep_sep {
                            output.push(sep_tok);
                        }
                    }
                    let mut sub_captures = captures.clone();
                    for (k, v) in captures {
                        if let CapturedFragment::Repeated(list) = v {
                            if idx < list.len() {
                                sub_captures.insert(k.clone(), CapturedFragment::Single(list[idx].clone()));
                            }
                        }
                    }
                    output.extend(self.transcribe_flat_template(rep_template, &sub_captures, call_span, expansion_id));
                }
                continue;
            }

            // Check for `$var` or `@var` metavariable substitution
            if (tok.kind == TokenKind::Dollar || tok.kind == TokenKind::At)
                && i + 1 < template.len()
                && template[i + 1].kind == TokenKind::Identifier
            {
                let var_tok = template[i + 1];
                let var_name = &self.source[var_tok.span.start as usize..var_tok.span.end as usize];
                if let Some(captured) = captures.get(var_name) {
                    match captured {
                        CapturedFragment::Single(sub_tokens) => {
                            output.extend(sub_tokens.clone());
                        }
                        CapturedFragment::Repeated(list) => {
                            for (idx, elem) in list.iter().enumerate() {
                                if idx > 0 {
                                    output.push(Token::new(TokenKind::Comma, call_span));
                                }
                                output.extend(elem.clone());
                            }
                        }
                    }
                    i += 2;
                    continue;
                }
            }

            let mut t = tok;
            t.span.ctxt = SyntaxContext(expansion_id);
            output.push(t);
            i += 1;
        }

        output
    }
}
