import re

with open('crates/mellis-semantic/src/typechecker.rs', 'r', encoding='utf-8') as f:
    code = f.read()

# 1. Stub comptime eval methods
code = re.sub(
    r'pub fn eval_comptime_stmt\(&mut self, stmt_id: mellis_ast::StmtId\) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> \{.*?\n    \}',
    r'''pub fn eval_comptime_stmt(&mut self, stmt_id: mellis_ast::StmtId) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> {
        Err(crate::comptime::ComptimeError::UnsupportedOperation("comptime expressions are not yet supported".to_string()))
    }''',
    code,
    flags=re.DOTALL
)

code = re.sub(
    r'pub fn eval_comptime_expr\(&mut self, expr_id: mellis_ast::ExprId\) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> \{.*?\n    \}',
    r'''pub fn eval_comptime_expr(&mut self, expr_id: mellis_ast::ExprId) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> {
        Err(crate::comptime::ComptimeError::UnsupportedOperation("comptime expressions are not yet supported".to_string()))
    }''',
    code,
    flags=re.DOTALL
)

# 2. Fix trait_impls.insert
old_insert = '''                    self.ctx.tables.trait_impls.insert(
                        crate::semantic_tables::ImplKey {
                            trait_id: Some(trait_sym),
                            self_type_def: self_sym,
                        },
                        *decl_id,
                    );'''
new_insert = '''                    self.ctx.tables.trait_impls
                        .entry(crate::semantic_tables::ImplKey {
                            trait_id: Some(trait_sym),
                            self_type_def: self_sym,
                        })
                        .or_insert_with(Vec::new)
                        .push(*decl_id);'''
code = code.replace(old_insert, new_insert)

# 3. Fix SemanticType::Struct/Enum instantiation and matching
# Add Vec::new() where we instantiate Struct/Enum with 2 args
code = re.sub(r'intern\(SemanticType::Struct\(([^,]+), ([^,]+)\)\)', r'intern(SemanticType::Struct(\1, \2, Vec::new()))', code)
code = re.sub(r'intern\(SemanticType::Enum\(([^,]+), ([^,]+)\)\)', r'intern(SemanticType::Enum(\1, \2, Vec::new()))', code)
# Fix literal `_` left behind from previous attempts if any
code = re.sub(r'SemanticType::Struct\(([^,]+), ([^,]+), _\)', r'SemanticType::Struct(\1, \2, Vec::new())', code)
code = re.sub(r'SemanticType::Enum\(([^,]+), ([^,]+), _\)', r'SemanticType::Enum(\1, \2, Vec::new())', code)

# Matching: add `_`
code = code.replace('SemanticType::Struct(sym_id, field_tys) => {', 'SemanticType::Struct(sym_id, field_tys, _) => {')
code = code.replace('SemanticType::Enum(sym_id, variant_tys) => {', 'SemanticType::Enum(sym_id, variant_tys, _) => {')
code = code.replace('SemanticType::Struct(_, field_tys) = self.ctx.types.get(struct_ty).clone()', 'SemanticType::Struct(_, field_tys, _) = self.ctx.types.get(struct_ty).clone()')
code = code.replace('SemanticType::Struct(sym_id, field_tys) = peeled_ty', 'SemanticType::Struct(sym_id, field_tys, _) = peeled_ty')
code = code.replace('SemanticType::Struct(sym_id, _) = peeled_ty', 'SemanticType::Struct(sym_id, _, _) = peeled_ty')
code = code.replace('SemanticType::Enum(sym_id, _) => {', 'SemanticType::Enum(sym_id, _, _) => {')
code = code.replace('SemanticType::Struct(sym_id, _) => {', 'SemanticType::Struct(sym_id, _, _) => {')

# 4. Fix Expr::Literal(tok) -> Expr::Literal(tok, _) in `get_expr_span_for_diag`
code = code.replace('Expr::Literal(tok) => Some(tok.span),', 'Expr::Literal(tok, _) => Some(tok.span),')

# 5. Fix `impl_decl_ids` in `trait_impls` block manually using balanced braces
def replace_trait_impls_loop(code):
    pos = 0
    while True:
        target = 'for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {'
        idx = code.find(target, pos)
        if idx == -1:
            break
        # Find matching closing brace
        brace_count = 0
        end_idx = -1
        start_search = idx + len(target) - 1 # The `{` character
        for i in range(start_search, len(code)):
            if code[i] == '{':
                brace_count += 1
            elif code[i] == '}':
                brace_count -= 1
                if brace_count == 0:
                    end_idx = i
                    break
        if end_idx != -1:
            # We found the block!
            block = code[idx:end_idx+1]
            new_block = block.replace(
                'for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {', 
                'for (impl_key, impl_decl_ids) in &self.ctx.tables.trait_impls {\n                        for &impl_decl_id in impl_decl_ids {'
            )
            # Add one more `}` before the end of the block, but keep the original indentation of the block end if possible
            # Wait, `end_idx` points to the `}` of the outer loop. We need to add one `}` just before it.
            # Actually, `}` \n `}` is fine.
            new_block = new_block[:-1] + '    }\n                    }'
            code = code[:idx] + new_block + code[end_idx+1:]
            pos = idx + len(new_block)
        else:
            pos = idx + len(target)
    return code

code = replace_trait_impls_loop(code)

with open('crates/mellis-semantic/src/typechecker.rs', 'w', encoding='utf-8') as f:
    f.write(code)
