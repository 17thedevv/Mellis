
import re

with open("CMakeLists.txt", "r", encoding="utf-8") as f:
    content = f.read()

test_target = """
add_executable(test_workspace_builder
    tests/test_workspace_builder.cpp
    ${CORE_SRC}
    ${FRONTEND_SRC}
    ${AST_SRC}
    ${SUPPORT_SRC}
)
target_include_directories(test_workspace_builder PRIVATE include)
target_link_libraries(test_workspace_builder PRIVATE ${LLVM_LIBS})
add_test(NAME WorkspaceBuilderTest COMMAND test_workspace_builder)
"""
if "test_workspace_builder" not in content:
    content += test_target

with open("CMakeLists.txt", "w", encoding="utf-8") as f:
    f.write(content)

print("Patched CMakeLists.txt with test_workspace_builder")

