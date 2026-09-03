import re

def main():
    file_path = "crates/mellis-semantic/src/typechecker.rs"
    with open(file_path, 'r', encoding='utf-8') as f:
        text = f.read()

    # Revert my previous patch to EnumVariant types in typechecker.rs since it was wrong
    # Let me just revert the patch_enum.py completely.
    # Actually, the patch_enum.py was trying to match:
    old_code = """                            let enum_ty = self.ctx.types.intern(SemanticType::Enum(sym_id, generic_args, variant_tys));
                            self.ctx.tables.symbol_types.insert(sym_id, enum_ty);
                            
                            if let mellis_ast::Decl::Enum { variants, .. } = &self.arena.decls[decl_id.0 as usize] {
                                for v in variants {
                                    if let Some(v_sym) = self.ctx.tables.decl_symbols.get(v).copied() {
                                        self.ctx.tables.symbol_types.insert(v_sym, enum_ty);
                                    }
                                }
                            }
                        }"""
    revert_code = """                            let enum_ty = self.ctx.types.intern(SemanticType::Enum(sym_id, generic_args, variant_tys));
                            self.ctx.tables.symbol_types.insert(sym_id, enum_ty);
                        }"""

    if old_code in text:
        text = text.replace(old_code, revert_code)

    # Now let's fix Expr::Call for EnumVariants!
    # I'll just write a script to insert the EnumVariant logic into Expr::Call.

    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(text)
    print("Done")

if __name__ == "__main__":
    main()
