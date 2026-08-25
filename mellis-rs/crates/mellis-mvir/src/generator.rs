use crate::mvir::*;
use mellis_ast::{AstArena, Item, Expr, Stmt, Decl};
use mellis_semantic::SemanticContext;
use std::collections::HashMap;

pub struct MvirGenerator<'a> {
    arena: &'a AstArena,
    ctx: &'a SemanticContext,
    source: &'a str,
    module: Module,
    
    // State during function generation
    current_function: Option<Function>,
    current_block: Option<BasicBlock>,
    next_label_id: u32,
    
    // Track local variables to their Alloca ValueId
    locals: HashMap<mellis_common::ids::SymbolId, ValueId>,
    lexical_scopes: Vec<Vec<mellis_common::ids::SymbolId>>,
    loop_scopes: Vec<usize>,
    loop_break_targets: Vec<LabelId>,
    loop_continue_targets: Vec<LabelId>,
}

impl<'a> MvirGenerator<'a> {
    pub fn new(arena: &'a AstArena, ctx: &'a SemanticContext, source: &'a str) -> Self {
        Self {
            arena,
            ctx,
            source,
            module: Module::new(),
            current_function: None,
            current_block: None,
            next_label_id: 0,
            locals: HashMap::new(),
            lexical_scopes: Vec::new(),
            loop_scopes: Vec::new(),
            loop_break_targets: Vec::new(),
            loop_continue_targets: Vec::new(),
        }
    }

    pub fn generate(mut self, items: &[Item]) -> Module {
        for instance in &self.ctx.mono_instances {
            self.generate_mono_instance(instance);
        }
        self.module
    }

    fn push_scope(&mut self) {
        self.lexical_scopes.push(Vec::new());
    }

    fn pop_scope_and_drop(&mut self, keep: Option<mellis_common::ids::SymbolId>) {
        if let Some(scope) = self.lexical_scopes.pop() {
            self.emit_drops_for_scope(&scope, keep);
        }
    }

    fn emit_drops_for_scope(&mut self, scope: &[mellis_common::ids::SymbolId], keep: Option<mellis_common::ids::SymbolId>) {
        for &sym in scope.iter().rev() {
            if Some(sym) == keep { continue; }
            if let Some(&val_id) = self.locals.get(&sym) {
                let ty_id = self.ctx.tables.symbol_types.get(&sym).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                if self.ctx.needs_drop(ty_id) {
                    self.push_inst(Instruction::Drop { value: Operand::Value(val_id) }, ty_id);
                }
            }
        }
    }

    fn emit_drops_up_to(&mut self, target_depth: usize, keep: Option<mellis_common::ids::SymbolId>) {
        for i in (target_depth..self.lexical_scopes.len()).rev() {
            let scope = self.lexical_scopes[i].clone();
            self.emit_drops_for_scope(&scope, keep);
        }
    }

    fn generate_item(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Var { name: _, initializer, pattern, .. } => {
                        let init_op = if let Some(init_expr) = initializer {
                            self.generate_expr(init_expr)
                        } else {
                            Operand::Number("0".to_string())
                        };
                        
                        if let Some(pat_id) = pattern {
                            self.bind_pattern(pat_id, init_op);
                        }
                    }
                    _ => {} // Functions are handled by generate_mono_instance
                }
            }
            Item::Stmt(stmt_id) => {
                self.generate_stmt(stmt_id);
            }
        }
    }

    fn bind_pattern(&mut self, pat_id: &mellis_ast::PatId, val_op: Operand) {
        match &self.arena.pats[pat_id.0 as usize] {
            mellis_ast::Pattern::Identifier { .. } => {
                if let Some(sym_id) = self.ctx.tables.pat_symbols.get(pat_id).copied() {
                    let ty_id = self.ctx.tables.symbol_types.get(&sym_id).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                    let alloca_val = self.push_inst(Instruction::Alloca, ty_id);
                    self.locals.insert(sym_id, alloca_val);
                    if let Some(scope) = self.lexical_scopes.last_mut() {
                        scope.push(sym_id);
                    }
                    
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(alloca_val),
                        value: val_op,
                    }, ty_id);
                }
            }
            mellis_ast::Pattern::Struct { fields, .. } => {
                let ty_id = self.ctx.tables.pat_types.get(pat_id).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                let resolved_ty = self.ctx.types.get(ty_id).clone();
                if let mellis_semantic::SemanticType::Struct(sym_id, _) = resolved_ty {
                    if let Some(decl_id) = self.ctx.symbol_table.get_symbol(sym_id).decl_id {
                        let decl = self.arena.decls[decl_id.0 as usize].clone();
                        if let mellis_ast::Decl::Struct { fields: struct_fields, .. } = &decl {
                            for field in fields {
                                if let Some(field_pat) = field.pattern {
                                    let field_name_str = &self.source[field.name.start as usize..field.name.end as usize];
                                    let mut field_idx = 0;
                                    for (idx, struct_field) in struct_fields.iter().enumerate() {
                                        let struct_field_name = &self.source[struct_field.name.start as usize..struct_field.name.end as usize];
                                        if field_name_str == struct_field_name {
                                            field_idx = idx;
                                            break;
                                        }
                                    }
                                    let field_ty = self.ctx.tables.pat_types.get(&field_pat).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                                    let extract_val = self.push_inst(Instruction::Extract {
                                        value: val_op.clone(),
                                        variant_idx: 0,
                                        field_idx: field_idx as u32,
                                    }, field_ty);
                                    self.bind_pattern(&field_pat, Operand::Value(extract_val));
                                }
                            }
                        }
                    }
                }
            }
            mellis_ast::Pattern::Tuple { elements, .. } => {
                for (idx, elem) in elements.iter().enumerate() {
                    let field_ty = self.ctx.tables.pat_types.get(elem).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                    let extract_val = self.push_inst(Instruction::Extract {
                        value: val_op.clone(),
                        variant_idx: 0,
                        field_idx: idx as u32,
                    }, field_ty);
                    self.bind_pattern(elem, Operand::Value(extract_val));
                }
            }
            mellis_ast::Pattern::Enum { fields: elements, .. } => {
                // TODO: enum extraction
                for elem in elements {
                    self.bind_pattern(elem, Operand::Number("0".to_string()));
                }
            }
            _ => {}
        }
    }

    fn generate_lvalue(&mut self, expr_id: &mellis_ast::ExprId) -> Operand {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        
        match expr {
            Expr::Identifier { .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id).copied() {
                    if let Some(&val_id) = self.locals.get(&sym_id) {
                        return Operand::Value(val_id);
                    }
                }
                Operand::Number("0".to_string())
            }
            Expr::Unary { op: mellis_ast::expr::UnaryOp::Deref, operand } => {
                self.generate_expr(operand)
            }
            _ => Operand::Number("0".to_string()),
        }
    }

    fn generate_mono_instance(&mut self, instance: &mellis_semantic::MonoInstance) {
        let decl = &self.arena.decls[instance.decl_id.0 as usize];
        if let Decl::Function { body, .. } = decl {
            let sym_id_opt = self.ctx.tables.decl_symbols.get(&instance.decl_id).copied();
            let mut fn_name = "func".to_string();
            let mut ret_ty_id = mellis_semantic::SemanticTypeId(0);
            
            if let Some(sym_id) = sym_id_opt {
                if (sym_id.0 as usize) < self.ctx.symbol_table.symbols.len() {
                    fn_name = self.ctx.symbol_table.symbols[sym_id.0 as usize].name.clone();
                }
                if let Some(fn_ty_id) = self.ctx.tables.symbol_types.get(&sym_id) {
                    if let mellis_semantic::SemanticType::Function { return_type, .. } = self.ctx.types.get(*fn_ty_id) {
                        ret_ty_id = *return_type;
                    }
                }
            }
            
            let mut suffix = String::new();
            if !instance.subst.is_empty() {
                suffix = "_mono".to_string();
            }
            let name_str = format!("{}{}", fn_name, suffix);
            let global_id = GlobalId {
                name: name_str,
                symbol_id: sym_id_opt,
            };
            
            let arg_count = if let Decl::Function { params, .. } = decl { params.len() } else { 0 };

            self.current_function = Some(Function {
                name: global_id,
                is_extern: matches!(decl, Decl::Extern { .. }),
                ret_ty: ret_ty_id,
                blocks: Vec::new(),
                values: Vec::new(),
                arg_count,
            });
            self.locals.clear();
            self.lexical_scopes.clear();
            self.loop_scopes.clear();
            self.loop_break_targets.clear();
            self.loop_continue_targets.clear();
            self.push_scope(); // Function root scope
            self.next_label_id = 0;
            
            let entry_label = self.new_label("entry");
            self.start_block(entry_label);
            
            if let Decl::Function { params, .. } = decl {
                for param_id in params {
                    if let Decl::Param { .. } = &self.arena.decls[param_id.0 as usize] {
                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(param_id).copied() {
                            let ty_id = self.ctx.tables.symbol_types.get(&sym_id).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                            let alloc_val = self.push_inst(Instruction::Alloca, ty_id);
                            self.locals.insert(sym_id, alloc_val);
                            self.lexical_scopes.last_mut().unwrap().push(sym_id);
                        }
                    }
                }
            }
            
            if let Some(body_stmt) = body {
                self.generate_stmt(body_stmt);
            }
            
            if let Some(mut block) = self.current_block.take() {
                if block.terminator.is_none() {
                    self.current_block = Some(block);
                    self.emit_drops_up_to(0, None);
                    block = self.current_block.take().unwrap();

                    let ret_val = if ret_ty_id == mellis_semantic::SemanticTypeId(0) {
                        None
                    } else {
                        Some(Operand::Number("0".to_string()))
                    };
                    block.terminator = Some(Terminator::Ret { value: ret_val });
                }
                self.current_function.as_mut().unwrap().blocks.push(block);
            }
            
            if let Some(func) = self.current_function.take() {
                self.module.functions.push(func);
            }
        }
    }

    fn generate_stmt(&mut self, stmt_id: &mellis_ast::StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        match stmt {
            Stmt::Expr { expr, .. } => {
                self.generate_expr(expr);
            }
            Stmt::Block { body, tail_expr } => {
                self.push_scope();
                for item in body {
                    self.generate_item(item);
                }
                if let Some(expr) = tail_expr {
                    self.generate_expr(expr);
                }
                self.pop_scope_and_drop(None);
            }
            Stmt::Return { value } => {
                let mut returned_sym = None;
                let val_operand = if let Some(expr_id) = value {
                    let expr = &self.arena.exprs[expr_id.0 as usize];
                    if let Expr::Identifier { .. } = expr {
                        if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id).copied() {
                            if self.locals.contains_key(&sym_id) {
                                returned_sym = Some(sym_id);
                            }
                        }
                    }
                    Some(self.generate_expr(expr_id))
                } else {
                    None
                };
                
                self.emit_drops_up_to(0, returned_sym);
                self.terminate_block(Terminator::Ret { value: val_operand });
            }
            Stmt::While { condition, body, .. } => {
                let cond_label = self.new_label("while_cond");
                let body_label = self.new_label("while_body");
                let end_label = self.new_label("while_end");

                self.terminate_block(Terminator::Br { target: cond_label.clone() });
                self.start_block(cond_label.clone());

                let cond_op = self.generate_expr(condition);
                self.terminate_block(Terminator::CondBr {
                    condition: cond_op,
                    true_target: body_label.clone(),
                    false_target: end_label.clone(),
                });

                self.start_block(body_label.clone());
                
                self.loop_scopes.push(self.lexical_scopes.len());
                self.loop_break_targets.push(end_label.clone());
                self.loop_continue_targets.push(cond_label.clone());
                
                self.generate_stmt(body);
                self.terminate_block(Terminator::Br { target: cond_label.clone() });

                self.loop_scopes.pop();
                self.loop_break_targets.pop();
                self.loop_continue_targets.pop();

                self.start_block(end_label.clone());
            }
            Stmt::Break { .. } => {
                if let (Some(&target_depth), Some(end_label)) = (self.loop_scopes.last(), self.loop_break_targets.last().cloned()) {
                    self.emit_drops_up_to(target_depth, None);
                    self.terminate_block(Terminator::Br { target: end_label });
                }
            }
            Stmt::Continue { .. } => {
                if let (Some(&target_depth), Some(cond_label)) = (self.loop_scopes.last(), self.loop_continue_targets.last().cloned()) {
                    self.emit_drops_up_to(target_depth, None);
                    self.terminate_block(Terminator::Br { target: cond_label });
                }
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let then_label = self.new_label("if_then");
                let else_label = self.new_label("if_else");
                let end_label = self.new_label("if_end");

                let cond_op = self.generate_expr(condition);
                self.terminate_block(Terminator::CondBr {
                    condition: cond_op,
                    true_target: then_label.clone(),
                    false_target: if else_branch.is_some() { else_label.clone() } else { end_label.clone() },
                });

                self.start_block(then_label.clone());
                self.generate_stmt(then_branch);
                self.terminate_block(Terminator::Br { target: end_label.clone() });

                if let Some(else_branch) = else_branch {
                    self.start_block(else_label.clone());
                    self.generate_stmt(else_branch);
                    self.terminate_block(Terminator::Br { target: end_label.clone() });
                }

                self.start_block(end_label.clone());
            }
            _ => {}
        }
    }

    fn generate_block_expr(&mut self, stmt_id: &mellis_ast::StmtId) -> Operand {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        if let Stmt::Block { body, tail_expr } = stmt {
            for item in body {
                self.generate_item(item);
            }
            if let Some(expr) = tail_expr {
                return self.generate_expr(expr);
            }
        } else {
            self.generate_stmt(stmt_id);
        }
        Operand::Number("0".to_string())
    }

    fn generate_expr(&mut self, expr_id: &mellis_ast::ExprId) -> Operand {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        let ty_id = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
        
        match expr {
            Expr::Literal(tok) => {
                match tok.kind {
                    mellis_lexer::TokenKind::IntegerLiteral => {
                        let text = self.source[tok.span.start as usize..tok.span.end as usize].to_string();
                        Operand::Number(text)
                    }
                    mellis_lexer::TokenKind::KwTrue => Operand::Boolean(true),
                    mellis_lexer::TokenKind::KwFalse => Operand::Boolean(false),
                    _ => Operand::Number("null".to_string()),
                }
            }
            Expr::Binary { op, left, right, .. } => {
                let left_op = self.generate_expr(left);
                let right_op = self.generate_expr(right);
                let inst = match op {
                    mellis_ast::expr::BinaryOp::Add => Instruction::Add { left: left_op, right: right_op },
                    mellis_ast::expr::BinaryOp::Sub => Instruction::Sub { left: left_op, right: right_op },
                    mellis_ast::expr::BinaryOp::Mul => Instruction::Mul { left: left_op, right: right_op },
                    _ => Instruction::Add { left: left_op, right: right_op },
                };
                let val_id = self.push_inst(inst, ty_id);
                Operand::Value(val_id)
            }
            Expr::Call { callee, args, .. } => {
                let callee_op = self.generate_expr(callee);
                let mut arg_ops = Vec::new();
                for arg in args {
                    arg_ops.push(self.generate_expr(&arg.value));
                }
                
                // Check if callee is an enum variant
                let is_variant = if let Some(sym_id) = self.ctx.tables.expr_symbols.get(callee) {
                    let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                    if let mellis_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                        Some((idx, symbol.decl_id.unwrap()))
                    } else { None }
                } else { None };

                if let Some((variant_idx, decl_id)) = is_variant {
                    let enum_sym_id = self.ctx.tables.decl_symbols.get(&decl_id).unwrap();
                    let enum_ty = self.ctx.tables.symbol_types.get(enum_sym_id).unwrap();
                    let call_val = self.push_inst(Instruction::Variant {
                        enum_ty: *enum_ty,
                        variant_idx,
                        args: arg_ops,
                    }, ty_id);
                    Operand::Value(call_val)
                } else {
                    let call_val = self.push_inst(Instruction::Call {
                        callee: callee_op,
                        args: arg_ops,
                    }, ty_id);
                    Operand::Value(call_val)
                }
            }
            Expr::Identifier { segments, .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id).copied() {
                    if let Some(&val_id) = self.locals.get(&sym_id) {
                        // Let's check if the local is a pointer. Wait, locals is a map to Alloca.
                        // Wait, what if sym_id is EnumVariant? Locals won't have it.
                    }
                    
                    let symbol = self.ctx.symbol_table.get_symbol(sym_id);
                    if let mellis_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                        if let Some(decl_id) = symbol.decl_id {
                            if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {
                                if let Some(enum_ty) = self.ctx.tables.symbol_types.get(enum_sym_id) {
                                    let variant_val = self.push_inst(Instruction::Variant {
                                        enum_ty: *enum_ty,
                                        variant_idx: idx,
                                        args: Vec::new(),
                                    }, ty_id);
                                    return Operand::Value(variant_val);
                                }
                            }
                        }
                    }
                    
                    if let Some(&val_id) = self.locals.get(&sym_id) {
                        let load_val = self.push_inst(Instruction::Load {
                            ptr: Operand::Value(val_id),
                        }, ty_id);
                        return Operand::Value(load_val);
                    }
                    
                    let sym_name = if (sym_id.0 as usize) < self.ctx.symbol_table.symbols.len() {
                        self.ctx.symbol_table.symbols[sym_id.0 as usize].name.clone()
                    } else {
                        format!("global_{}", segments[0].start)
                    };
                    return Operand::Global(GlobalId {
                        name: sym_name,
                        symbol_id: Some(sym_id),
                    });
                }
                Operand::Number("0".to_string())
            }
            Expr::Assign { op: _, lvalue, value } => {
                let ptr_op = self.generate_lvalue(lvalue);
                let val_op = self.generate_expr(value);
                let val_ty_id = self.ctx.tables.expr_types.get(value).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                self.push_inst(Instruction::Store {
                    ptr: ptr_op,
                    value: val_op,
                }, val_ty_id);
                Operand::Number("0".to_string())
            }
            Expr::StructInit { fields, .. } => {
                let struct_alloca = self.push_inst(Instruction::Alloca, ty_id);
                for field in fields {
                    let val_op = self.generate_expr(&field.value);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(struct_alloca),
                        value: val_op,
                    }, mellis_semantic::SemanticTypeId(0));
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(struct_alloca),
                }, ty_id);
                Operand::Value(load_val)
            }
            Expr::TupleLiteral { elements } => {
                let tuple_alloca = self.push_inst(Instruction::Alloca, ty_id);
                for elem in elements {
                    let val_op = self.generate_expr(elem);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(tuple_alloca),
                        value: val_op,
                    }, mellis_semantic::SemanticTypeId(0));
                }
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(tuple_alloca),
                }, ty_id);
                Operand::Value(load_val)
            }
            Expr::Index { base, index } => {
                let base_op = self.generate_expr(base);
                let index_op = self.generate_expr(index);
                
                // Get base type to extract array length
                let base_ty_id = self.ctx.tables.expr_types.get(base).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                let base_ty = self.ctx.types.get(base_ty_id).clone();
                
                if let mellis_semantic::SemanticType::Array(_, len) = base_ty {
                    let len_op = Operand::Number(len.to_string());
                    self.push_inst(Instruction::BoundsCheck {
                        index: index_op.clone(),
                        len: len_op,
                    }, mellis_semantic::SemanticTypeId(0));
                }
                
                // Emitting the actual element access logic will be done by pointer offset later.
                // For now we just return an invalid operand or 0 since full code-gen for elements isn't done.
                Operand::Number("0".to_string())
            }
            Expr::Match { subject, arms } => {
                let subject_op = self.generate_expr(subject);
                let match_ty_id = self.ctx.tables.expr_types.get(expr_id).copied().unwrap_or(mellis_semantic::SemanticTypeId(0));
                
                let result_alloca = self.push_inst(Instruction::Alloca, match_ty_id);
                let end_label = self.new_label("match_end");
                
                let mut next_arm_label = self.new_label("match_arm");
                
                for (i, arm) in arms.iter().enumerate() {
                    self.terminate_block(Terminator::Br { target: next_arm_label.clone() });
                    self.start_block(next_arm_label.clone());
                    
                    next_arm_label = if i == arms.len() - 1 {
                        self.new_label("match_unreachable") // We assume exhaustion
                    } else {
                        self.new_label("match_arm")
                    };
                    
                    let is_match = self.generate_pat_match(&arm.pattern, &subject_op);
                    
                    let body_label = self.new_label("match_body");
                    self.terminate_block(Terminator::CondBr {
                        condition: is_match,
                        true_target: body_label.clone(),
                        false_target: next_arm_label.clone(),
                    });
                    
                    self.start_block(body_label);
                    
                    self.bind_pat_vars(&arm.pattern, &subject_op);
                    let body_op = self.generate_block_expr(&arm.body);
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(result_alloca),
                        value: body_op,
                    }, match_ty_id);
                    
                    self.terminate_block(Terminator::Br { target: end_label.clone() });
                }
                
                self.start_block(next_arm_label);
                self.terminate_block(Terminator::Unreachable);
                
                self.start_block(end_label);
                
                let load_val = self.push_inst(Instruction::Load {
                    ptr: Operand::Value(result_alloca),
                }, match_ty_id);
                
                Operand::Value(load_val)
            }
            Expr::Unary { op, operand } => {
                use mellis_ast::expr::UnaryOp;
                let inst = match op {
                    UnaryOp::Ref => {
                        let op_val = self.generate_lvalue(operand);
                        Instruction::Borrow { is_rw: false, base: op_val }
                    }
                    UnaryOp::RefMut => {
                        let op_val = self.generate_lvalue(operand);
                        Instruction::Borrow { is_rw: true, base: op_val }
                    }
                    _ => {
                        let _op_val = self.generate_expr(operand);
                        return Operand::Number("0".to_string())
                    }
                };
                let val_id = self.push_inst(inst, ty_id);
                Operand::Value(val_id)
            }
            _ => Operand::Number("0".to_string())
        }
    }
    
    // --- Helpers ---
    
    fn new_label(&mut self, prefix: &str) -> LabelId {
        let id = self.next_label_id;
        self.next_label_id += 1;
        LabelId {
            name: format!("{}{}", prefix, id),
        }
    }
    
    fn start_block(&mut self, label: LabelId) {
        if let Some(mut block) = self.current_block.take() {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator::Br { target: label.clone() });
            }
            self.current_function.as_mut().unwrap().blocks.push(block);
        }
        self.current_block = Some(BasicBlock {
            label,
            insts: Vec::new(),
            terminator: None,
        });
    }
    
    fn push_inst(&mut self, inst: Instruction, ty: mellis_semantic::SemanticTypeId) -> ValueId {
        let func = self.current_function.as_mut().expect("Must be in a function");
        let val_id = ValueId(func.values.len() as u32);
        func.values.push(ValueData { inst, ty, span: None });
        
        if let Some(block) = &mut self.current_block {
            block.insts.push(val_id);
        }
        val_id
    }
    
    fn terminate_block(&mut self, term: Terminator) {
        if let Some(mut block) = self.current_block.take() {
            if block.terminator.is_none() {
                block.terminator = Some(term);
            }
            self.current_function.as_mut().unwrap().blocks.push(block);
        }
    }
    
    fn generate_pat_match(&mut self, pat: &mellis_ast::PatId, subject: &Operand) -> Operand {
        use mellis_ast::Pattern;
        let pattern = &self.arena.pats[pat.0 as usize];
        match pattern {
            Pattern::Wildcard | Pattern::Identifier { .. } => {
                Operand::Boolean(true)
            }
            Pattern::Enum { path, .. } => {
                let mut variant_idx = 0;
                // Simple lookup of variant index by name string
                if let Some(name_span) = path.last() {
                    let name_str = &self.source[name_span.start as usize..name_span.end as usize];
                    // Very simplistic: just find the enum variant symbol by matching its name suffix
                    for (sym_id, decl_id) in self.ctx.tables.symbol_decls.iter() {
                        let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                        if let mellis_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                            if symbol.name.ends_with(name_str) {
                                variant_idx = idx;
                                break;
                            }
                        }
                    }
                }
                
                let tag_val = self.push_inst(Instruction::Tag {
                    value: subject.clone(),
                }, mellis_semantic::SemanticTypeId(0));
                
                let expected_tag = Operand::Number(variant_idx.to_string());
                
                let eq_val = self.push_inst(Instruction::Eq {
                    left: Operand::Value(tag_val),
                    right: expected_tag,
                }, mellis_semantic::SemanticTypeId(0));
                
                Operand::Value(eq_val)
            }
            _ => Operand::Boolean(false),
        }
    }
    
    fn bind_pat_vars(&mut self, pat: &mellis_ast::PatId, subject: &Operand) {
        use mellis_ast::Pattern;
        let pattern = &self.arena.pats[pat.0 as usize];
        match pattern {
            Pattern::Identifier { segments, .. } => {
                if let Some(sym_id) = self.ctx.tables.pat_symbols.get(pat).copied() {
                    let symbol = self.ctx.symbol_table.get_symbol(sym_id);
                    if let mellis_semantic::SymbolKind::EnumVariant(_) = symbol.kind {
                        // Enum variant matches do not bind a local variable.
                    } else if let Operand::Value(val_id) = subject {
                        self.locals.insert(sym_id, *val_id);
                        if let Some(scope) = self.lexical_scopes.last_mut() {
                            scope.push(sym_id);
                        }
                    }
                }
            }
            Pattern::Enum { fields, path } => {
                let mut variant_idx = 0;
                if let Some(name_span) = path.last() {
                    let name_str = &self.source[name_span.start as usize..name_span.end as usize];
                    for (sym_id, decl_id) in self.ctx.tables.symbol_decls.iter() {
                        let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                        if let mellis_semantic::SymbolKind::EnumVariant(idx) = symbol.kind {
                            if symbol.name.ends_with(name_str) {
                                variant_idx = idx;
                                break;
                            }
                        }
                    }
                }
                
                for (field_idx, field) in fields.iter().enumerate() {
                    let extracted = self.push_inst(Instruction::Extract {
                        value: subject.clone(),
                        variant_idx,
                        field_idx: field_idx as u32,
                    }, mellis_semantic::SemanticTypeId(0)); // We'd need actual field ty
                    
                    self.bind_pat_vars(field, &Operand::Value(extracted));
                }
            }
            _ => {}
        }
    }
}
