import json
import os

files = [
    r'C:\Users\84387\.gemini\antigravity-ide\brain\6c4591be-b06d-4e30-86ae-f5d0fae43bac\.system_generated\logs\transcript_full.jsonl',
    r'C:\Users\84387\.gemini\antigravity-ide\brain\cb08b0ac-e016-4a49-96ba-8bfc6148b194\.system_generated\logs\transcript_full.jsonl',
    r'C:\Users\84387\.gemini\antigravity-ide\brain\ff6a2b9c-a552-4dc6-93c3-af366e7b6885\.system_generated\logs\transcript_full.jsonl'
]

for file in files:
    if not os.path.exists(file):
        continue
    with open(file, 'r', encoding='utf-8') as f:
        for line in f:
            try:
                data = json.loads(line)
                if 'tool_calls' in data:
                    for call in data['tool_calls']:
                        name = call.get('name')
                        if name in ['default_api:write_to_file', 'default_api:replace_file_content', 'default_api:multi_replace_file_content']:
                            args = call.get('arguments', {})
                            if 'llvm_codegen.rs' in args.get('TargetFile', ''):
                                print(f"Found tool call {name} in {file}")
            except:
                pass
