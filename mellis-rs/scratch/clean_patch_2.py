import re

with open('crates/mellis-semantic/src/typechecker.rs', 'r', encoding='utf-8') as f:
    code = f.read()

# Fix trait_impls.insert using regex
# Find `self.ctx.tables.trait_impls.insert(` up to `*decl_id,\n                    );`
pat_insert = re.compile(
    r'self\.ctx\.tables\.trait_impls\.insert\(\s+crate::semantic_tables::ImplKey \{\s+trait_id: Some\(trait_sym\),\s+self_type_def: self_sym,\s+\},\s+\*decl_id,\s+\);',
    re.DOTALL
)
replacement_insert = r'''self.ctx.tables.trait_impls
                                            .entry(crate::semantic_tables::ImplKey {
                                                trait_id: Some(trait_sym),
                                                self_type_def: self_sym,
                                            })
                                            .or_insert_with(Vec::new)
                                            .push(*decl_id);'''
code = pat_insert.sub(replacement_insert, code)

# Fix SemanticType::Struct(s, _) -> SemanticType::Struct(s, _, _)
code = re.sub(r'SemanticType::Struct\(([^,]+), _\)', r'SemanticType::Struct(\1, _, _)', code)

# Fix Expr::Literal(tok)
code = re.sub(r'Expr::Literal\(tok\)', r'Expr::Literal(tok, _)', code)

with open('crates/mellis-semantic/src/typechecker.rs', 'w', encoding='utf-8') as f:
    f.write(code)
