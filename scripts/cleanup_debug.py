"""
Fix TypeChecker.cpp by removing all debug/trace lines:
- Lines that start with identifiers/expressions followed by "); fflush(stderr);" patterns
- These are broken argument lines left over from removed fprintf calls
"""

with open('src/MiddleEnd/TypeChecker.cpp', 'r', encoding='utf-8') as f:
    lines = f.readlines()

import re

new_lines = []
i = 0
while i < len(lines):
    line = lines[i]
    stripped = line.strip()
    
    # Pattern 1: lines ending with ); fflush(stderr); that are leftover args
    # These look like: "    node.symbolId, std::string(...), typeTable.size(), ...); fflush(stderr);"
    if re.search(r'fflush\(stderr\)\s*;', stripped) and not stripped.startswith('fprintf') and not stripped.startswith('//'):
        i += 1
        continue
    
    # Pattern 2: standalone fflush lines
    if stripped == 'fflush(stderr);':
        i += 1
        continue
    
    # Pattern 3: fprintf debug lines  
    if stripped.startswith('fprintf(stderr') and 'DEBUG' in stripped:
        i += 1
        continue
    
    # Pattern 4: std::cout debug lines
    if 'std::cout << "[TypeChecker]' in stripped:
        i += 1
        continue
    
    new_lines.append(line)
    i += 1

with open('src/MiddleEnd/TypeChecker.cpp', 'w', encoding='utf-8') as f:
    f.writelines(new_lines)

print(f'Done: {len(lines)} -> {len(new_lines)} lines (removed {len(lines)-len(new_lines)} lines)')
