import os

def restore():
    with open('recovered_tc2.rs', 'r', encoding='utf-8') as f:
        content = f.read()
        
    content = content.replace('// MISSING LINE 1916\\n// MISSING LINE 1917\\n// MISSING LINE 1918\\n// MISSING LINE 1919', '                                                                                    ));\\n                                                                                    if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }\\n                                                                                    self.ctx.diagnostics.push(diag);\\n                                                                                }')

    content = content.split('fn enforce_mutability')[0]
    
    enforce = """    fn enforce_mutability(&mut self, expr_id: &mellis_ast::ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        if let mellis_ast::Expr::Identifier { segments, .. } = expr {
            if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {
                let symbol = self.ctx.symbol_table.get_symbol(*sym_id);
                if matches!(symbol.kind, crate::symbol::SymbolKind::Constant) {
                    let mut diag = mellis_common::diagnostic::Diagnostic::error("Cannot mutate immutable variable");
                    if let Some(&span) = segments.first() {
                        diag.span = Some(span);
                    }
                    self.ctx.diagnostics.push(diag);
                }
            }
        }
    }
}
"""
    content += enforce
    
    lines = content.split('\\n')
    clean_lines = []
    last_empty = False
    for line in lines:
        if line.strip() == '':
            if not last_empty:
                clean_lines.append('')
            last_empty = True
        else:
            clean_lines.append(line)
            last_empty = False

    with open('crates/mellis-semantic/src/typechecker.rs', 'w', encoding='utf-8') as f:
        f.write('\\n'.join(clean_lines))

restore()
