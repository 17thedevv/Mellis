import json
import os

files = [
    r'C:\Users\84387\.gemini\antigravity-ide\brain\683e43a9-f8e9-4856-9958-e3be5a65065c\.system_generated\logs\transcript_full.jsonl',
    r'C:\Users\84387\.gemini\antigravity-ide\brain\6c4591be-b06d-4e30-86ae-f5d0fae43bac\.system_generated\logs\transcript_full.jsonl',
    r'C:\Users\84387\.gemini\antigravity-ide\brain\cb08b0ac-e016-4a49-96ba-8bfc6148b194\.system_generated\logs\transcript_full.jsonl',
    r'C:\Users\84387\.gemini\antigravity-ide\brain\ff6a2b9c-a552-4dc6-93c3-af366e7b6885\.system_generated\logs\transcript_full.jsonl'
]

latest_content = None
max_size = 0

for file in files:
    if not os.path.exists(file):
        continue
    with open(file, 'r', encoding='utf-8') as f:
        for line in f:
            try:
                data = json.loads(line)
                if 'tool_calls' in data:
                    for call in data['tool_calls']:
                        if call.get('name') == 'default_api:write_to_file':
                            args = call.get('arguments', {})
                            if 'llvm_codegen.rs' in args.get('TargetFile', ''):
                                code = args.get('CodeContent', '')
                                if len(code) > max_size:
                                    max_size = len(code)
                                    latest_content = code
            except:
                pass

if latest_content:
    with open('recovered_llvm_codegen.rs', 'w', encoding='utf-8') as f:
        f.write(latest_content)
    print(f"Recovered {max_size} bytes!")
else:
    print("No write_to_file found for llvm_codegen.rs.")
