import os

def replace_in_file(path, old, new):
    if not os.path.exists(path): return
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    content = content.replace(old, new)
    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)

replace_in_file(r"d:\fdlang\compiler\src\FrontEnd\Parser.cpp", 'std::cerr << "[DEBUG]', 'extern bool g_quiet; if (!g_quiet) std::cerr << "[DEBUG]')
