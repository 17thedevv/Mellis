
import re

with open("CMakeLists.txt", "r", encoding="utf-8") as f:
    content = f.read()

content = content.replace("""add_executable(test_workspace_builder
    tests/test_workspace_builder.cpp
    ${CORE_SRC}
    ${FRONTEND_SRC}
    ${AST_SRC}
    ${SUPPORT_SRC}
)""", """add_executable(test_workspace_builder
    tests/test_workspace_builder.cpp
    ${CORE_SRC}
    ${FRONTEND_SRC}
    ${AST_SRC}
    ${SUPPORT_SRC}
    ${MIDDLEEND_SRC}
    ${BACKEND_SRC}
    ${MLIB_SRC}
)""")

with open("CMakeLists.txt", "w", encoding="utf-8") as f:
    f.write(content)

print("Patched CMakeLists.txt with all SRCs")

