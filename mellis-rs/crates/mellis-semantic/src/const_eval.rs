use std::collections::{HashMap, HashSet};
use mellis_ast::{AstArena, Expr, ExprId, DeclId, Stmt, StmtId, Item, Decl};
use mellis_common::ids::{SymbolId, Span};
use mellis_common::source::SourceManager;
use crate::SemanticContext;
use crate::symbol::SymbolKind;

fn get_span_text<'a>(source_manager: &'a SourceManager, span: Span) -> &'a str {
    if let Some(file) = source_manager.get_file(span.file_id) {
        let start = span.start as usize;
        let end = span.end as usize;
        if start <= end && end <= file.source.len() {
            return &file.source[start..end];
        }
    }
    ""
}

/// Checks if an expression satisfies the pure, non-mutating const-evaluable admission rules.
/// If an expression requires imperative control flow (loops, local mutation), it must be
/// encapsulated within an embedded `comptime { ... }` block (Expr::Comptime).
pub fn is_const_evaluable(
    arena: &AstArena,
    ctx: &SemanticContext,
    source_manager: &SourceManager,
    expr_id: ExprId,
) -> Result<(), String> {
    let expr = &arena.exprs[expr_id.0 as usize];
    match expr {
        Expr::Literal(..) => Ok(()),

        Expr::Binary { left, right, .. } => {
            is_const_evaluable(arena, ctx, source_manager, *left)?;
            is_const_evaluable(arena, ctx, source_manager, *right)
        }

        Expr::Unary { operand, .. } => {
            is_const_evaluable(arena, ctx, source_manager, *operand)
        }

        Expr::Cast { expr, .. } => {
            is_const_evaluable(arena, ctx, source_manager, *expr)
        }

        Expr::TupleLiteral { elements } => {
            for elem in elements {
                is_const_evaluable(arena, ctx, source_manager, *elem)?;
            }
            Ok(())
        }

        Expr::ArrayLiteral { elements } => {
            for elem in elements {
                is_const_evaluable(arena, ctx, source_manager, *elem)?;
            }
            Ok(())
        }

        Expr::StructInit { fields, .. } => {
            for field in fields {
                is_const_evaluable(arena, ctx, source_manager, field.value)?;
            }
            Ok(())
        }

        Expr::Member { object, .. } | Expr::TupleIndex { object, .. } => {
            is_const_evaluable(arena, ctx, source_manager, *object)
        }

        Expr::Index { base, index } => {
            is_const_evaluable(arena, ctx, source_manager, *base)?;
            is_const_evaluable(arena, ctx, source_manager, *index)
        }

        Expr::Sizeof { .. } | Expr::Alignof { .. } => Ok(()),

        Expr::Comptime { .. } => {
            // An embedded comptime block has its own effect-containment rules in the VM.
            Ok(())
        }

        Expr::Identifier { segments, .. } => {
            if let Some(sym_id) = ctx.tables.expr_symbols.get(&expr_id) {
                let symbol = ctx.symbol_table.get_symbol(*sym_id);
                match symbol.kind {
                    SymbolKind::Constant | SymbolKind::EnumVariant(_) => Ok(()),
                    _ => Err(format!(
                        "variable '{}' is not a compile-time constant; only const items may be referenced in const expressions",
                        symbol.name
                    )),
                }
            } else if let Some(last_seg) = segments.last() {
                let seg_name = get_span_text(source_manager, *last_seg);
                if !seg_name.is_empty() {
                    if let Some(sym_id) = ctx.symbol_table.lookup_with_ctxt(seg_name, last_seg.ctxt, crate::ScopeId(0)) {
                        let symbol = ctx.symbol_table.get_symbol(sym_id);
                        match symbol.kind {
                            SymbolKind::Constant | SymbolKind::EnumVariant(_) => Ok(()),
                            _ => Err(format!(
                                "variable '{}' is not a compile-time constant",
                                symbol.name
                            )),
                        }
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        }

        Expr::Call { .. } => {
            Err("function calls in const initializers are forbidden; wrap the call in 'comptime { ... }'".to_string())
        }

        Expr::Assign { .. } => {
            Err("assignment is forbidden in const expressions; use 'comptime { ... }' for imperative computation".to_string())
        }

        Expr::Await { .. } => {
            Err("await expressions are forbidden at compile time".to_string())
        }

        Expr::Lambda { .. } => {
            Err("closures and lambdas cannot be evaluated in raw const expressions".to_string())
        }

        _ => Ok(()),
    }
}

/// Recursively collects all constant symbol IDs referenced by an expression.
pub fn collect_const_dependencies(
    arena: &AstArena,
    ctx: &SemanticContext,
    source_manager: &SourceManager,
    expr_id: ExprId,
    deps: &mut HashSet<SymbolId>,
) {
    let expr = &arena.exprs[expr_id.0 as usize];
    match expr {
        Expr::Identifier { segments, .. } => {
            if let Some(sym_id) = ctx.tables.expr_symbols.get(&expr_id) {
                let symbol = ctx.symbol_table.get_symbol(*sym_id);
                if matches!(symbol.kind, SymbolKind::Constant) {
                    deps.insert(*sym_id);
                }
            } else if let Some(last_seg) = segments.last() {
                let seg_name = get_span_text(source_manager, *last_seg);
                if !seg_name.is_empty() {
                    if let Some(sym_id) = ctx.symbol_table.lookup_with_ctxt(seg_name, last_seg.ctxt, crate::ScopeId(0)) {
                        let symbol = ctx.symbol_table.get_symbol(sym_id);
                        if matches!(symbol.kind, SymbolKind::Constant) {
                            deps.insert(sym_id);
                        }
                    }
                }
            }
        }

        Expr::Binary { left, right, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *left, deps);
            collect_const_dependencies(arena, ctx, source_manager, *right, deps);
        }

        Expr::Unary { operand, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *operand, deps);
        }

        Expr::Cast { expr, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *expr, deps);
        }

        Expr::TupleLiteral { elements } => {
            for elem in elements {
                collect_const_dependencies(arena, ctx, source_manager, *elem, deps);
            }
        }

        Expr::ArrayLiteral { elements } => {
            for elem in elements {
                collect_const_dependencies(arena, ctx, source_manager, *elem, deps);
            }
        }

        Expr::StructInit { fields, .. } => {
            for field in fields {
                collect_const_dependencies(arena, ctx, source_manager, field.value, deps);
            }
        }

        Expr::Member { object, .. } | Expr::TupleIndex { object, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *object, deps);
        }

        Expr::Index { base, index } => {
            collect_const_dependencies(arena, ctx, source_manager, *base, deps);
            collect_const_dependencies(arena, ctx, source_manager, *index, deps);
        }

        Expr::Comptime { body } => {
            collect_const_dependencies_stmt(arena, ctx, source_manager, *body, deps);
        }

        Expr::Call { callee, args, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *callee, deps);
            for arg in args {
                collect_const_dependencies(arena, ctx, source_manager, arg.value, deps);
            }
        }

        Expr::Assign { lvalue, value, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *lvalue, deps);
            collect_const_dependencies(arena, ctx, source_manager, *value, deps);
        }

        _ => {}
    }
}

/// Recursively collects constant dependencies referenced within statements.
pub fn collect_const_dependencies_stmt(
    arena: &AstArena,
    ctx: &SemanticContext,
    source_manager: &SourceManager,
    stmt_id: StmtId,
    deps: &mut HashSet<SymbolId>,
) {
    let stmt = &arena.stmts[stmt_id.0 as usize];
    match stmt {
        Stmt::Block { body, tail_expr } => {
            for item in body {
                match item {
                    Item::Decl(decl_id) => {
                        let decl = &arena.decls[decl_id.0 as usize];
                        if let Decl::Var { initializer: Some(init_expr), .. } = decl {
                            collect_const_dependencies(arena, ctx, source_manager, *init_expr, deps);
                        }
                    }
                    Item::Stmt(s_id) => {
                        collect_const_dependencies_stmt(arena, ctx, source_manager, *s_id, deps);
                    }
                }
            }
            if let Some(tail) = tail_expr {
                collect_const_dependencies(arena, ctx, source_manager, *tail, deps);
            }
        }
        Stmt::Expr { expr, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *expr, deps);
        }
        Stmt::If { condition, then_branch, else_branch } => {
            collect_const_dependencies(arena, ctx, source_manager, *condition, deps);
            collect_const_dependencies_stmt(arena, ctx, source_manager, *then_branch, deps);
            if let Some(else_b) = else_branch {
                collect_const_dependencies_stmt(arena, ctx, source_manager, *else_b, deps);
            }
        }
        Stmt::While { condition, body, .. } => {
            collect_const_dependencies(arena, ctx, source_manager, *condition, deps);
            collect_const_dependencies_stmt(arena, ctx, source_manager, *body, deps);
        }
        Stmt::Return { value: Some(val) } => {
            collect_const_dependencies(arena, ctx, source_manager, *val, deps);
        }
        Stmt::Unsafe { body } | Stmt::Comptime { body } => {
            collect_const_dependencies_stmt(arena, ctx, source_manager, *body, deps);
        }
        _ => {}
    }
}

#[derive(Debug)]
pub struct ConstCycleError {
    pub span: Span,
    pub cycle: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}

/// Sorts a set of top-level constant declarations into topological evaluation order.
/// Detects cyclic dependencies and returns `ConstCycleError` with the cycle chain.
pub fn sort_constants_topological(
    const_decls: &[(DeclId, ExprId, Span)],
    arena: &AstArena,
    ctx: &SemanticContext,
    source_manager: &SourceManager,
) -> Result<Vec<(DeclId, ExprId)>, ConstCycleError> {
    let mut decl_to_sym = HashMap::new();
    let mut sym_to_decl = HashMap::new();
    let mut decl_spans = HashMap::new();

    for (decl_id, _, span) in const_decls {
        decl_spans.insert(*decl_id, *span);
        if let Some(&sym_id) = ctx.tables.decl_symbols.get(decl_id) {
            decl_to_sym.insert(*decl_id, sym_id);
            sym_to_decl.insert(sym_id, *decl_id);
        }
    }

    // Build dependency adjacency list: decl_id -> Vec<dep_decl_id>
    let mut adj: HashMap<DeclId, Vec<DeclId>> = HashMap::new();
    for (decl_id, init_expr, _) in const_decls {
        let mut deps = HashSet::new();
        collect_const_dependencies(arena, ctx, source_manager, *init_expr, &mut deps);
        let mut dep_decls = Vec::new();
        for dep_sym in deps {
            if let Some(&dep_decl) = sym_to_decl.get(&dep_sym) {
                if dep_decl != *decl_id {
                    dep_decls.push(dep_decl);
                }
            }
        }
        adj.insert(*decl_id, dep_decls);
    }

    // DFS with cycle detection
    let mut state: HashMap<DeclId, VisitState> = HashMap::new();
    for (decl_id, _, _) in const_decls {
        state.insert(*decl_id, VisitState::Unvisited);
    }

    let mut order = Vec::new();
    let mut call_stack = Vec::new();

    for (decl_id, _, span) in const_decls {
        if state.get(decl_id) == Some(&VisitState::Unvisited) {
            let mut cycle_path = Vec::new();
            if dfs_visit(
                *decl_id,
                &adj,
                &mut state,
                &mut call_stack,
                &mut order,
                &mut cycle_path,
            ) {
                let cycle_names = cycle_path
                    .iter()
                    .map(|did| {
                        if let Some(&sym) = decl_to_sym.get(did) {
                            ctx.symbol_table.get_symbol(sym).name.clone()
                        } else {
                            format!("decl_{}", did.0)
                        }
                    })
                    .collect();
                return Err(ConstCycleError {
                    span: *span,
                    cycle: cycle_names,
                });
            }
        }
    }

    // Map ordered decls back to (DeclId, ExprId)
    let decl_map: HashMap<DeclId, ExprId> = const_decls
        .iter()
        .map(|(d, e, _)| (*d, *e))
        .collect();

    let result = order
        .into_iter()
        .map(|d| (d, decl_map[&d]))
        .collect();

    Ok(result)
}

fn dfs_visit(
    u: DeclId,
    adj: &HashMap<DeclId, Vec<DeclId>>,
    state: &mut HashMap<DeclId, VisitState>,
    call_stack: &mut Vec<DeclId>,
    order: &mut Vec<DeclId>,
    cycle_path: &mut Vec<DeclId>,
) -> bool {
    state.insert(u, VisitState::Visiting);
    call_stack.push(u);

    if let Some(neighbors) = adj.get(&u) {
        for &v in neighbors {
            match state.get(&v) {
                Some(VisitState::Visiting) => {
                    // Cycle detected!
                    let start_idx = call_stack.iter().position(|&x| x == v).unwrap_or(0);
                    cycle_path.extend_from_slice(&call_stack[start_idx..]);
                    cycle_path.push(v); // close the cycle loop
                    return true;
                }
                Some(VisitState::Unvisited) => {
                    if dfs_visit(v, adj, state, call_stack, order, cycle_path) {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }

    call_stack.pop();
    state.insert(u, VisitState::Visited);
    order.push(u);
    false
}
