import json

with open('src/MiddleEnd/TypeChecker.cpp', 'r', encoding='utf-8') as f:
    lines = f.read().splitlines()

def apply_chunk(lines, start, end, target, replacement, allow_multiple=False):
    target_lines = target.split('\n')
    repl_lines = replacement.split('\n')
    # search for target in lines[start-1:end]
    start_idx = max(0, start - 1)
    end_idx = min(len(lines), end)
    
    match_idx = -1
    for i in range(start_idx, end_idx - len(target_lines) + 2):
        match = True
        for j in range(len(target_lines)):
            if i+j >= len(lines) or lines[i+j] != target_lines[j]:
                match = False
                break
        if match:
            match_idx = i
            break
            
    if match_idx != -1:
        new_lines = lines[:match_idx] + repl_lines + lines[match_idx + len(target_lines):]
        return new_lines
    return lines

with open(r'C:\Users\84387\.gemini\antigravity-ide\brain\eb3811d8-1f29-4346-af13-1c1fabb58f43\.system_generated\logs\transcript_full.jsonl', 'r', encoding='utf-8') as f:
    for line in f:
        step = json.loads(line)
        if step.get('type') == 'PLANNER_RESPONSE':
            if 'tool_calls' in step:
                for call in step['tool_calls']:
                    if 'replace_file_content' in call['name']:
                        args = call.get('args', {})
                        if 'TypeChecker.cpp' in args.get('TargetFile', ''):
                            chunks = args.get('ReplacementChunks', [])
                            if not chunks and 'ReplacementContent' in args:
                                chunks = [args]
                            # Apply chunks in reverse order to not mess up line numbers if possible
                            # But wait, replace_file_content applies them sequentially in reality.
                            for chunk in chunks:
                                start = chunk.get('StartLine', 1)
                                end = chunk.get('EndLine', len(lines))
                                target = chunk.get('TargetContent', '')
                                repl = chunk.get('ReplacementContent', '')
                                lines = apply_chunk(lines, start, end, target, repl)

with open('src/MiddleEnd/TypeChecker.cpp', 'w', encoding='utf-8') as f:
    f.write('\n'.join(lines) + '\n')
print('Finished applying edits! Lines:', len(lines))
