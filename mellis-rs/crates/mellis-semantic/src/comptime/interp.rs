use std::collections::HashMap;
use mellis_ast::{
    AstArena, BinaryOp, Decl, Expr, ExprId, Item, Pattern, Stmt, StmtId, UnaryOp,
};
use mellis_common::ids::SymbolId;
use mellis_common::Span;
use crate::SemanticContext;
use super::value::{ComptimeError, ComptimeValue, IntWidth, FloatWidth};
use super::reflect::ComptimeReflection;

#[derive(Debug, Clone)]
pub enum ComptimeControlFlow {
    Continue,
    Break,
    Return(ComptimeValue),
    Value(ComptimeValue),
}

pub struct ComptimeContext {
    pub scopes: Vec<HashMap<String, ComptimeValue>>,
    pub consts: HashMap<SymbolId, ComptimeValue>,
    pub step_count: usize,
    pub max_steps: usize,
    pub call_stack_depth: usize,
    pub max_call_depth: usize,
    pub call_stack: Vec<(Option<SymbolId>, Span)>,
}

impl Default for ComptimeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ComptimeContext {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            consts: HashMap::new(),
            step_count: 0,
            max_steps: 1_000_000,
            call_stack_depth: 0,
            max_call_depth: 512,
            call_stack: Vec::new(),
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn set_var(&mut self, name: String, val: ComptimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, val);
        }
    }

    pub fn assign_var(&mut self, name: &str, val: ComptimeValue) -> Result<(), ComptimeError> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), val);
                return Ok(());
            }
        }
        // If not found in scopes, set in innermost
        self.set_var(name.to_string(), val);
        Ok(())
    }

    pub fn get_var(&self, name: &str) -> Option<ComptimeValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    pub fn tick(&mut self) -> Result<(), ComptimeError> {
        self.step_count += 1;
        if self.step_count > self.max_steps {
            Err(ComptimeError::StepLimitExceeded(self.max_steps))
        } else {
            Ok(())
        }
    }
}

pub struct ComptimeEvaluator<'a> {
    pub arena: &'a AstArena,
    pub sem_ctx: &'a SemanticContext,
    pub source: &'a str,
}

impl<'a> ComptimeEvaluator<'a> {
    pub fn new(arena: &'a AstArena, sem_ctx: &'a SemanticContext, source: &'a str) -> Self {
        Self {
            arena,
            sem_ctx,
            source,
        }
    }

    pub fn eval_expr(
        &self,
        expr_id: ExprId,
        ctx: &mut ComptimeContext,
    ) -> Result<ComptimeValue, ComptimeError> {
        ctx.tick()?;
        let expr = &self.arena.exprs[expr_id.0 as usize];

        match expr {
            Expr::Literal(tok) => self.eval_literal(tok),
            Expr::Identifier { segments, .. } => {
                if segments.is_empty() {
                    return Ok(ComptimeValue::Unit);
                }
                let name = if segments.len() == 1 {
                    let span = segments[0];
                    self.source[span.start as usize..span.end as usize].to_string()
                } else {
                    segments
                        .iter()
                        .map(|s| &self.source[s.start as usize..s.end as usize])
                        .collect::<Vec<_>>()
                        .join("::")
                };

                if let Some(val) = ctx.get_var(&name) {
                    return Ok(val);
                }

                let sym_id_opt = self.sem_ctx.tables.expr_symbols.get(&expr_id).copied()
                    .or_else(|| self.sem_ctx.symbol_table.lookup(&name, crate::ScopeId(0)));

                if let Some(sym_id) = sym_id_opt {
                    if let Some(val) = ctx.consts.get(&sym_id) {
                        return Ok(val.clone());
                    }
                    let symbol = self.sem_ctx.symbol_table.get_symbol(sym_id);
                    if let Some(decl_id) = symbol.decl_id {
                        let decl = &self.arena.decls[decl_id.0 as usize];
                        if let Decl::Var { is_const: true, initializer: Some(init), .. } = decl {
                            let val = self.eval_expr(*init, ctx)?;
                            ctx.consts.insert(sym_id, val.clone());
                            return Ok(val);
                        }
                    }
                }

                Err(ComptimeError::SymbolNotFound(name))
            }
            Expr::Binary { op, left, right } => {
                let left_val = self.eval_expr(*left, ctx)?;
                // Short-circuit logical ops
                match op {
                    BinaryOp::LogicAnd => {
                        if !left_val.as_bool().unwrap_or(false) {
                            return Ok(ComptimeValue::Bool(false));
                        }
                        let right_val = self.eval_expr(*right, ctx)?;
                        return Ok(ComptimeValue::Bool(right_val.as_bool().unwrap_or(false)));
                    }
                    BinaryOp::LogicOr => {
                        if left_val.as_bool().unwrap_or(false) {
                            return Ok(ComptimeValue::Bool(true));
                        }
                        let right_val = self.eval_expr(*right, ctx)?;
                        return Ok(ComptimeValue::Bool(right_val.as_bool().unwrap_or(false)));
                    }
                    _ => {}
                }

                let right_val = self.eval_expr(*right, ctx)?;
                match op {
                    BinaryOp::Add => left_val.add(&right_val),
                    BinaryOp::Sub => left_val.sub(&right_val),
                    BinaryOp::Mul => left_val.mul(&right_val),
                    BinaryOp::Div => left_val.div(&right_val),
                    BinaryOp::Mod => left_val.rem(&right_val),
                    BinaryOp::BitAnd => left_val.bit_and(&right_val),
                    BinaryOp::BitOr => left_val.bit_or(&right_val),
                    BinaryOp::BitXor => left_val.bit_xor(&right_val),
                    BinaryOp::LShift => left_val.shl(&right_val),
                    BinaryOp::RShift => left_val.shr(&right_val),
                    BinaryOp::Eq => Ok(ComptimeValue::Bool(left_val.cmp_eq(&right_val))),
                    BinaryOp::Ne => Ok(ComptimeValue::Bool(!left_val.cmp_eq(&right_val))),
                    BinaryOp::Lt => Ok(ComptimeValue::Bool(left_val.cmp_lt(&right_val)?)),
                    BinaryOp::Le => Ok(ComptimeValue::Bool(left_val.cmp_le(&right_val)?)),
                    BinaryOp::Gt => Ok(ComptimeValue::Bool(left_val.cmp_gt(&right_val)?)),
                    BinaryOp::Ge => Ok(ComptimeValue::Bool(left_val.cmp_ge(&right_val)?)),
                    BinaryOp::LogicAnd => Ok(ComptimeValue::Bool(left_val.as_bool().unwrap_or(false) && right_val.as_bool().unwrap_or(false))),
                    BinaryOp::LogicOr => Ok(ComptimeValue::Bool(left_val.as_bool().unwrap_or(false) || right_val.as_bool().unwrap_or(false))),
                    BinaryOp::Range | BinaryOp::RangeInc => {
                        Err(ComptimeError::UnsupportedOperation("range operators in comptime".to_string()))
                    }
                }
            }
            Expr::Unary { op, operand } => {
                let val = self.eval_expr(*operand, ctx)?;
                match op {
                    UnaryOp::Neg => val.neg(),
                    UnaryOp::Not | UnaryOp::BitNot => val.not(),
                    UnaryOp::PostInc => {
                        let new_val = val.add(&ComptimeValue::i32(1))?;
                        if let Expr::Identifier { segments, .. } = &self.arena.exprs[operand.0 as usize] {
                            let name = &self.source[segments[0].start as usize..segments[0].end as usize];
                            ctx.assign_var(name, new_val)?;
                        }
                        Ok(val)
                    }
                    UnaryOp::PostDec => {
                        let new_val = val.sub(&ComptimeValue::i32(1))?;
                        if let Expr::Identifier { segments, .. } = &self.arena.exprs[operand.0 as usize] {
                            let name = &self.source[segments[0].start as usize..segments[0].end as usize];
                            ctx.assign_var(name, new_val)?;
                        }
                        Ok(val)
                    }
                    UnaryOp::Ref | UnaryOp::RefMut => {
                        // Intra-comptime values can be passed directly by value
                        Ok(val)
                    }
                    UnaryOp::Deref | UnaryOp::DerefMut => Ok(val),
                }
            }
            Expr::Assign { op: _, lvalue, value } => {
                let rval = self.eval_expr(*value, ctx)?;
                if let Expr::Identifier { segments, .. } = &self.arena.exprs[lvalue.0 as usize] {
                    let name = &self.source[segments[0].start as usize..segments[0].end as usize];
                    ctx.assign_var(name, rval.clone())?;
                    Ok(rval)
                } else {
                    Err(ComptimeError::UnsupportedOperation("complex lvalue assignment".to_string()))
                }
            }
            Expr::TupleLiteral { elements } => {
                let mut evaluated = Vec::new();
                for elem in elements {
                    evaluated.push(self.eval_expr(*elem, ctx)?);
                }
                Ok(ComptimeValue::Tuple(evaluated))
            }
            Expr::ArrayLiteral { elements } => {
                let mut evaluated = Vec::new();
                for elem in elements {
                    evaluated.push(self.eval_expr(*elem, ctx)?);
                }
                Ok(ComptimeValue::Array {
                    elements: evaluated,
                    elem_ty: None,
                })
            }
            Expr::TupleIndex { object, index } => {
                let obj = self.eval_expr(*object, ctx)?;
                match obj {
                    ComptimeValue::Tuple(elems) => {
                        elems.get(*index as usize).cloned().ok_or_else(|| {
                            ComptimeError::Custom(format!("tuple index out of bounds: {}", index))
                        })
                    }
                    _ => Err(ComptimeError::TypeMismatch("expected tuple for tuple index".to_string())),
                }
            }
            Expr::Index { base, index } => {
                let base_val = self.eval_expr(*base, ctx)?;
                let idx_val = self.eval_expr(*index, ctx)?;
                let idx = idx_val.as_usize().ok_or_else(|| {
                    ComptimeError::TypeMismatch("expected non-negative integer for array index".to_string())
                })?;

                match base_val {
                    ComptimeValue::Array { elements, .. } => {
                        elements.get(idx).cloned().ok_or_else(|| {
                            ComptimeError::Custom(format!("array index out of bounds: {} >= {}", idx, elements.len()))
                        })
                    }
                    _ => Err(ComptimeError::TypeMismatch("expected array for index expression".to_string())),
                }
            }
            Expr::Comptime { body } => {
                let flow = self.eval_stmt(*body, ctx)?;
                match flow {
                    ComptimeControlFlow::Return(v) | ComptimeControlFlow::Value(v) => Ok(v),
                    _ => Ok(ComptimeValue::Unit),
                }
            }
            Expr::Sizeof { target_type } => {
                // Size of target type using ComptimeReflection
                let size = ComptimeReflection::sizeof(crate::ty::SemanticTypeId(target_type.0), self.sem_ctx);
                Ok(ComptimeValue::usize(size))
            }
            Expr::Alignof { target_type } => {
                let align = ComptimeReflection::alignof(crate::ty::SemanticTypeId(target_type.0), self.sem_ctx);
                Ok(ComptimeValue::usize(align))
            }
            Expr::Call { callee, args, .. } => {
                let (func_body, params) = self.resolve_function_target(*callee, ctx)?;
                
                if ctx.call_stack_depth >= ctx.max_call_depth {
                    return Err(ComptimeError::RecursionLimitExceeded(ctx.max_call_depth));
                }

                // Evaluate arguments before entering new frame
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_expr(arg.value, ctx)?);
                }

                ctx.call_stack_depth += 1;
                ctx.enter_scope();

                // Bind parameters
                for (param_decl_id, val) in params.iter().zip(arg_vals.into_iter()) {
                    if let Decl::Param { name, .. } = &self.arena.decls[param_decl_id.0 as usize] {
                        let param_name = self.source[name.start as usize..name.end as usize].to_string();
                        ctx.set_var(param_name, val);
                    }
                }

                let flow = self.eval_stmt(func_body, ctx);
                ctx.exit_scope();
                ctx.call_stack_depth -= 1;

                match flow? {
                    ComptimeControlFlow::Return(v) | ComptimeControlFlow::Value(v) => Ok(v),
                    _ => Ok(ComptimeValue::Unit),
                }
            }
            Expr::Match { subject, arms, .. } => {
                let subject_val = self.eval_expr(*subject, ctx)?;
                for arm in arms {
                    ctx.enter_scope();
                    let matches = self.match_pattern(&arm.pattern, &subject_val, ctx)?;
                    if matches {
                        let res = self.eval_stmt(arm.body, ctx);
                        ctx.exit_scope();
                        match res? {
                            ComptimeControlFlow::Return(v) | ComptimeControlFlow::Value(v) => return Ok(v),
                            _ => return Ok(ComptimeValue::Unit),
                        }
                    }
                    ctx.exit_scope();
                }
                Err(ComptimeError::Custom("non-exhaustive match in comptime".to_string()))
            }
            Expr::Try { expr: inner, .. } => {
                let val = self.eval_expr(*inner, ctx)?;
                // If val is Option::None or Result::Err, early return
                match &val {
                    ComptimeValue::Enum { variant_name, payload, .. } => {
                        if variant_name == "None" || variant_name == "Err" {
                            return Err(ComptimeError::Custom("early return via '?' in comptime".to_string()));
                        }
                        if variant_name == "Some" || variant_name == "Ok" {
                            if let Some(first) = payload.first() {
                                return Ok(first.clone());
                            }
                        }
                    }
                    _ => {}
                }
                Ok(val)
            }
            _ => Err(ComptimeError::UnsupportedOperation("expression kind in comptime".to_string())),
        }
    }

    pub fn eval_stmt(
        &self,
        stmt_id: StmtId,
        ctx: &mut ComptimeContext,
    ) -> Result<ComptimeControlFlow, ComptimeError> {
        ctx.tick()?;
        let stmt = &self.arena.stmts[stmt_id.0 as usize];

        match stmt {
            Stmt::Block { body, tail_expr } => {
                ctx.enter_scope();
                for item in body {
                    match item {
                        Item::Decl(decl_id) => {
                            self.eval_decl(*decl_id, ctx)?;
                        }
                        Item::Stmt(s) => {
                            let flow = self.eval_stmt(*s, ctx)?;
                            if !matches!(flow, ComptimeControlFlow::Continue) {
                                ctx.exit_scope();
                                return Ok(flow);
                            }
                        }
                    }
                }
                let res = if let Some(tail) = tail_expr {
                    let val = self.eval_expr(*tail, ctx)?;
                    ComptimeControlFlow::Value(val)
                } else {
                    ComptimeControlFlow::Continue
                };
                ctx.exit_scope();
                Ok(res)
            }
            Stmt::Expr { expr, .. } => {
                let val = self.eval_expr(*expr, ctx)?;
                Ok(ComptimeControlFlow::Value(val))
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_val = self.eval_expr(*condition, ctx)?;
                if cond_val.as_bool().unwrap_or(false) {
                    self.eval_stmt(*then_branch, ctx)
                } else if let Some(else_b) = else_branch {
                    self.eval_stmt(*else_b, ctx)
                } else {
                    Ok(ComptimeControlFlow::Continue)
                }
            }
            Stmt::While { condition, body, .. } => {
                while self.eval_expr(*condition, ctx)?.as_bool().unwrap_or(false) {
                    let flow = self.eval_stmt(*body, ctx)?;
                    match flow {
                        ComptimeControlFlow::Break => break,
                        ComptimeControlFlow::Continue | ComptimeControlFlow::Value(_) => {}
                        ComptimeControlFlow::Return(v) => return Ok(ComptimeControlFlow::Return(v)),
                    }
                }
                Ok(ComptimeControlFlow::Continue)
            }
            Stmt::Return { value } => {
                let val = if let Some(v) = value {
                    self.eval_expr(*v, ctx)?
                } else {
                    ComptimeValue::Unit
                };
                Ok(ComptimeControlFlow::Return(val))
            }
            Stmt::Break { .. } => Ok(ComptimeControlFlow::Break),
            Stmt::Continue { .. } => Ok(ComptimeControlFlow::Continue),
            _ => Ok(ComptimeControlFlow::Continue),
        }
    }

    pub fn eval_decl(
        &self,
        decl_id: mellis_ast::DeclId,
        ctx: &mut ComptimeContext,
    ) -> Result<(), ComptimeError> {
        let decl = &self.arena.decls[decl_id.0 as usize];
        if let Decl::Var { name, initializer, .. } = decl {
            let var_name = self.source[name.start as usize..name.end as usize].to_string();
            let val = if let Some(init) = initializer {
                self.eval_expr(*init, ctx)?
            } else {
                ComptimeValue::Unit
            };
            ctx.set_var(var_name, val);
        }
        Ok(())
    }

    fn eval_literal(&self, tok: &mellis_lexer::Token) -> Result<ComptimeValue, ComptimeError> {
        let span = tok.span;
        let text = &self.source[span.start as usize..span.end as usize];
        match tok.kind {
            mellis_lexer::TokenKind::IntegerLiteral => {
                let clean_text = text.replace('_', "");
                let (val, width) = if clean_text.starts_with("0x") || clean_text.starts_with("0X") {
                    let n = i128::from_str_radix(&clean_text[2..], 16).map_err(|_| ComptimeError::IntegerOverflow)?;
                    (n, IntWidth::I32)
                } else if clean_text.starts_with("0b") || clean_text.starts_with("0B") {
                    let n = i128::from_str_radix(&clean_text[2..], 2).map_err(|_| ComptimeError::IntegerOverflow)?;
                    (n, IntWidth::I32)
                } else if clean_text.starts_with("0o") || clean_text.starts_with("0O") {
                    let n = i128::from_str_radix(&clean_text[2..], 8).map_err(|_| ComptimeError::IntegerOverflow)?;
                    (n, IntWidth::I32)
                } else {
                    let n: i128 = clean_text.parse().map_err(|_| ComptimeError::IntegerOverflow)?;
                    (n, IntWidth::I32)
                };
                Ok(ComptimeValue::Int { val, width })
            }
            mellis_lexer::TokenKind::FloatLiteral => {
                let clean_text = text.replace('_', "");
                let n: f64 = clean_text.parse().map_err(|_| ComptimeError::Custom("invalid float".to_string()))?;
                Ok(ComptimeValue::Float {
                    val: n,
                    width: FloatWidth::F64,
                })
            }
            mellis_lexer::TokenKind::KwTrue => Ok(ComptimeValue::Bool(true)),
            mellis_lexer::TokenKind::KwFalse => Ok(ComptimeValue::Bool(false)),
            mellis_lexer::TokenKind::StringLiteral | mellis_lexer::TokenKind::RawStringLiteral => {
                let trimmed = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
                    &text[1..text.len() - 1]
                } else {
                    text
                };
                Ok(ComptimeValue::Str(trimmed.to_string()))
            }
            mellis_lexer::TokenKind::CharLiteral => {
                let c = text.chars().nth(1).unwrap_or('\0');
                Ok(ComptimeValue::Char(c))
            }
            _ => Err(ComptimeError::UnsupportedOperation(format!("literal {:?}", tok.kind))),
        }
    }

    fn resolve_function_target(
        &self,
        callee: ExprId,
        _ctx: &ComptimeContext,
    ) -> Result<(StmtId, Vec<mellis_ast::DeclId>), ComptimeError> {
        if let Some(sym_id) = self.sem_ctx.tables.expr_symbols.get(&callee) {
            let symbol = self.sem_ctx.symbol_table.get_symbol(*sym_id);
            if let Some(decl_id) = symbol.decl_id {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { body: Some(body), params, .. } => {
                        return Ok((*body, params.clone()));
                    }
                    Decl::Extern { .. } => {
                        return Err(ComptimeError::ForbiddenSideEffect(format!("extern function '{}' cannot be called in comptime", symbol.name)));
                    }
                    _ => {}
                }
            }
        }
        Err(ComptimeError::UnsupportedOperation("calling indirect or unresolved function in comptime".to_string()))
    }

    fn match_pattern(
        &self,
        pat_id: &mellis_ast::PatId,
        val: &ComptimeValue,
        ctx: &mut ComptimeContext,
    ) -> Result<bool, ComptimeError> {
        let pat = &self.arena.pats[pat_id.0 as usize];
        match pat {
            Pattern::Wildcard => Ok(true),
            Pattern::Identifier { segments } => {
                if segments.len() == 1 {
                    let name = self.source[segments[0].start as usize..segments[0].end as usize].to_string();
                    ctx.set_var(name, val.clone());
                    Ok(true)
                } else {
                    Ok(true)
                }
            }
            Pattern::Literal(tok) => {
                let lit_val = self.eval_literal(tok)?;
                Ok(lit_val == *val)
            }
            _ => Ok(false),
        }
    }
}
