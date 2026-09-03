import json
import re

def extract_typechecker():
    path = r"C:\Users\84387\.gemini\antigravity-ide\brain\7f4891c9-efc5-4d64-a8a7-931a21786638\.system_generated\logs\transcript_full.jsonl"
    lines_map = {}
    
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            try:
                data = json.loads(line)
                if 'content' in data and type(data['content']) == str:
                    content = data['content']
                    if 'crates/mellis-semantic/src/typechecker.rs' in content:
                        # Extract the lines formatted as "123: code"
                        for m in re.finditer(r'^(\d+):\s(.*)$', content, re.MULTILINE):
                            line_num = int(m.group(1))
                            line_str = m.group(2)
                            lines_map[line_num] = line_str
            except:
                pass
                
    with open('d:/fdlang/mellis-rs/recovered_tc.rs', 'w', encoding='utf-8') as f:
        for i in range(1, max(lines_map.keys()) + 1 if lines_map else 1):
            f.write(lines_map.get(i, f"// MISSING LINE {i}") + '\n')
            
    print(f"Recovered {len(lines_map)} lines.")

extract_typechecker()
