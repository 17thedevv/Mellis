
import re

with open("CMakeLists.txt", "r", encoding="utf-8") as f:
    content = f.read()

content = re.sub(r"src/Core/SourceManager\.cpp", "${CORE_SRC}", content)

with open("CMakeLists.txt", "w", encoding="utf-8") as f:
    f.write(content)

print("Patched CMakeLists.txt")

