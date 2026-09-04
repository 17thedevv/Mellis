import re

with open('d:/fdlang/mellis-rs/crates/mellis-semantic/src/typechecker.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Remove the function definition completely using a regular expression
pattern = r'^\s*/// Walk an AST type.*?^    pub fn check_bounds_for_call'
content = re.sub(pattern, '    pub fn check_bounds_for_call', content, flags=re.MULTILINE | re.DOTALL)

# Remove the calls to extract_generic_subst
content = re.sub(r'^\s*self\.extract_generic_subst\(.*?\);\n', '', content, flags=re.MULTILINE)
content = re.sub(r'^\s*println!\(\"DEBUG: extract_generic_subst.*?;\n', '', content, flags=re.MULTILINE)
content = re.sub(r'^\s*println!\(\"extract_generic_subst.*?;\n', '', content, flags=re.MULTILINE)

with open('d:/fdlang/mellis-rs/crates/mellis-semantic/src/typechecker.rs', 'w', encoding='utf-8') as f:
    f.write(content)
