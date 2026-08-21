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
        }
    }

    pub fn generate(mut self, items: &[Item]) -> Module {
        for instance in &self.ctx.mono_instances {
            self.generate_mono_instance(instance);
        }
        self.module
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
                    
                    self.push_inst(Instruction::Store {
                        ptr: Operand::Value(alloca_val),
                        value: val_op,
                    }, ty_id);
                }
            }
            mellis_ast::Pattern::Struct { fields, .. } => {
                for field in fields {
                    if let Some(field_pat) = field.pattern {
                        self.bind_pattern(&field_pat, Operand::Number("0".to_string()));
                    }
                }
            }
            mellis_ast::Pattern::Tuple { elements, .. } | mellis_ast::Pattern::Enum { fields: elements, .. } => {
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
            
            self.current_function = Some(Function {
                name: global_id,
                blocks: Vec::new(),
                values: Vec::new(),
                ret_ty: ret_ty_id,
            });
            self.locals.clear();
            self.next_label_id = 0;
            
            let entry_label = self.new_label("entry");
            self.start_block(entry_label);
            
            if let Some(body_stmt) = body {
                self.generate_stmt(body_stmt);
            }
            
            if let Some(mut block) = self.current_block.take() {
                if block.terminator.is_none() {
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
                for item in body {
                    self.generate_item(item);
                }
                if let Some(expr) = tail_expr {
                    self.generate_expr(expr);
                }
            }
            Stmt::Return { value } => {
                let val_operand = if let Some(expr) = value {
                    Some(self.generate_expr(expr))
                } else {
                    None
                };
                self.terminate_block(Terminator::Ret { value: val_operand });
            }
            // Add if, while, for loops here later
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
                Operand::Value(struct_alloca)
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
        func.values.push(ValueData { inst, ty });
        
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
