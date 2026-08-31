import os
import glob
import re

files = glob.glob('crates/mellis-driver/tests/ui/*.ms')
for file in files:
    with open(file, 'r') as f:
        content = f.read()
    
    # 1. Remove _Marker<T> struct definition
    content = re.sub(r'struct _Marker<T>\s*\{\s*\}', '', content)
    
    # 2. Remove end and marker from VecIter struct
    content = re.sub(r'struct VecIter<T>\s*\{\s*ptr: \*rw T,\s*end: \*rw T,\s*marker: _Marker<T>,\s*\}', 'struct VecIter<T> {\n    ptr: *rw T,\n}', content)
    content = re.sub(r'struct VecIter<T>\s*\{\s*ptr: \*rw T,\s*\}', 'struct VecIter<T> {\n    ptr: *rw T,\n}', content)
    
    # 3. Replace iter_mut with simple return
    content = re.sub(r'fn iter_mut\(self: &rw Vec<T>\) -> VecIter<T>\s*\{.*?(?:return.*?;).*?\}', 'fn iter_mut(self: &rw Vec<T>) -> VecIter<T> {\n        dec rw iter: VecIter<T>;\n        iter.ptr = self.data;\n        return iter;\n    }', content, flags=re.DOTALL)
    
    with open(file, 'w') as f:
        f.write(content)
