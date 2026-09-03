import os
import re

ast_dir = r"d:\fdlang\mellis-rs\crates\mellis-ast\src"
files = ["decl.rs", "expr.rs", "stmt.rs", "ty.rs", "pat.rs"]

print("=== AST Nodes ===")
for f in files:
    path = os.path.join(ast_dir, f)
    if os.path.exists(path):
        with open(path, 'r', encoding='utf-8') as file:
            content = file.read()
            # Use basic block extraction
            for block in content.split("pub enum "):
                if not block.strip() or "{" not in block: continue
                name = block.split("{")[0].strip()
                name = name.split("<")[0].strip() # remove generics
                print(f"\nEnum: {name}")
                body = block.split("{")[1].split("}")[0]
                lines = body.split("\n")
                for line in lines:
                    line = line.strip()
                    if not line or line.startswith("//") or line.startswith("#"): continue
                    # Extract variant name
                    variant = line.split("(")[0].split("{")[0].strip().strip(",")
                    if variant:
                        print(f"  - {variant}")
