import os
import re

def fix():
    with open('crates/mellis-semantic/src/typechecker.rs', 'r', encoding='utf-8') as f:
        content = f.read()

    # 1. Fix line 944 missing lines
    target1 = """                            if ret_name == "Self" {
                                let mut diag = Diagnostic::error(format!(
            }
            (_, SemanticType::DynTrait(trait_sym)) => {"""
    
    rep1 = """                            if ret_name == "Self" {
                                let mut diag = Diagnostic::error(format!(
                                    "Trait `{}` cannot be made into an object because method `{}` returns `Self`",
                                    self.ctx.symbol_table.get_symbol(trait_sym).name,
                                    method_name
                                ));
                                if let Some(sp) = span { diag = diag.with_span(sp); }
                                self.ctx.diagnostics.push(diag);
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    pub fn is_coerceable(&mut self, from_ty: SemanticTypeId, to_ty: SemanticTypeId) -> bool {
        let from = self.ctx.types.get(from_ty);
        let to = self.ctx.types.get(to_ty);
        match (from, to) {
            (_, SemanticType::DynTrait(trait_sym)) => {"""
    
    if target1 in content:
        content = content.replace(target1, rep1)
    else:
        # Try a regex if exact match fails
        content = re.sub(
            r'if ret_name == "Self" \{\s*let mut diag = Diagnostic::error\(format!\(\s*\}\s*\(_, SemanticType::DynTrait\(trait_sym\)\) => \{',
            rep1,
            content
        )

    # 2. Fix line 2440 mismatched closing delimiter
    # 2440: if !self.ctx.tables.trait_impls.contains_key(&impl_key) {
    # Let's find this section in git_tc.rs to see what's missing.
    with open('git_tc.rs', 'r', encoding='utf-8') as f:
        git_content = f.read()
    
    # We will just write out the content so far and check cargo to pinpoint the next issue.
    with open('crates/mellis-semantic/src/typechecker.rs', 'w', encoding='utf-8') as f:
        f.write(content)

fix()
