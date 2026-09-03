import re

with open('crates/mellis-semantic/src/typechecker.rs', 'r', encoding='utf-8') as f:
    code = f.read()

# 1. Update unify_and_report signature and body
old_func = '''    pub fn unify_and_report(&mut self, expected: SemanticTypeId, actual: SemanticTypeId, span: mellis_common::ids::Span, context: &str) -> SemanticTypeId {
        if let Err(msg) = self.unify(expected, actual) {
            let diag = crate::Diagnostic::error(format!(
                "{}: {}\\n  Expected: {}\\n  Found: {}",
                context, msg, expected.0, actual.0
            )).with_span(span);
            self.ctx.diagnostics.push(diag);
            self.ctx.types.intern(SemanticType::Error)
        } else {
            expected
        }
    }'''

new_func = '''    pub fn unify_and_report(&mut self, expected: SemanticTypeId, actual: SemanticTypeId, span: mellis_common::ids::Span, context: &str) -> Result<SemanticTypeId, ()> {
        if let Err(msg) = self.unify(expected, actual) {
            let diag = crate::Diagnostic::error(format!(
                "{}: {}\\n  Expected: {}\\n  Found: {}",
                context, msg, expected.0, actual.0
            )).with_span(span);
            self.ctx.diagnostics.push(diag);
            Err(())
        } else {
            Ok(expected)
        }
    }'''

code = code.replace(old_func, new_func)

out = []
lines = code.split('\n')
i = 0
while i < len(lines):
    line = lines[i]
    if 'let check_ty = self.unify_and_report' in line:
        # Extract arguments
        prefix = line[:line.find('let check_ty')]
        args = line[line.find('(')+1:line.rfind(')')]
        
        # Check next lines for `if check_ty == ...`
        if i + 3 < len(lines) and 'if check_ty == self.ctx.types.intern(SemanticType::Error)' in lines[i+1]:
            # Replace 3 lines
            out.append(f'{prefix}if self.unify_and_report({args}).is_err() {{')
            out.append(f'{prefix}    return self.ctx.types.intern(SemanticType::Error);')
            out.append(f'{prefix}}}')
            i += 4
            continue
    out.append(line)
    i += 1

with open('crates/mellis-semantic/src/typechecker.rs', 'w', encoding='utf-8') as f:
    f.write('\n'.join(out))
