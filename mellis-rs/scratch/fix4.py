import os

def fix():
    with open('crates/mellis-semantic/src/typechecker.rs', 'r', encoding='utf-8') as f:
        content = f.read()

    target = """                                                        let mut diag = Diagnostic::error(format!(

                                                            "The type `{}` does not implement trait `{}` (required by generic parameter `{}`)",

                                                            type_name, trait_name, gp_name

                                                        ));

                                                        if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }

                                                            .with_span(gp.name)

                                                        );"""
    
    rep = """                                                        let mut diag = Diagnostic::error(format!(

                                                            "The type `{}` does not implement trait `{}` (required by generic parameter `{}`)",

                                                            type_name, trait_name, gp_name

                                                        ));

                                                        if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }

                                                        self.ctx.diagnostics.push(diag);"""
    
    if target in content:
        content = content.replace(target, rep)
    else:
        # Fallback to lines replace if exact spacing mismatch
        lines = content.split('\\n')
        for i in range(len(lines)):
            if 'if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }' in lines[i]:
                if i+2 < len(lines) and '.with_span(gp.name)' in lines[i+2]:
                    lines[i+2] = ''
                if i+4 < len(lines) and ');' in lines[i+4]:
                    lines[i+4] = '                                                        self.ctx.diagnostics.push(diag);'
        content = '\\n'.join(lines)

    with open('crates/mellis-semantic/src/typechecker.rs', 'w', encoding='utf-8') as f:
        f.write(content)

fix()
