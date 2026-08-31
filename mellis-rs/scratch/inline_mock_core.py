import os
import glob
import re

files = glob.glob('crates/mellis-driver/tests/ui/try_*.ms')
for file in files:
    with open(file, 'r') as f:
        content = f.read()
    
    if 'import "mock_core";' in content:
        content = content.replace('import "mock_core";', 'enum Error { E }\nenum Result<T, E> {\n    Ok(T),\n    Err(E),\n}')
    
    with open(file, 'w') as f:
        f.write(content)
