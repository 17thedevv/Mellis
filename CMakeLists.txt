# Yêu cầu phiên bản CMake tối thiểu
cmake_minimum_required(VERSION 3.20)

# Khai báo tên dự án và ngôn ngữ sử dụng
project(mellis VERSION 1.0.0 LANGUAGES CXX C)

# Ép buộc sử dụng C++17
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ==========================================
# PHẦN 1: TÌM VÀ CẤU HÌNH LLVM
# ==========================================
list(APPEND CMAKE_PREFIX_PATH "D:/Programs/LLVM-DEV")
find_package(LLVM REQUIRED CONFIG)

include_directories(${LLVM_INCLUDE_DIRS})
link_directories(${LLVM_LIBRARY_DIRS})

link_directories("C:/Program Files/Microsoft Visual Studio/18/Community/DIA SDK/lib/amd64")
add_definitions(${LLVM_DEFINITIONS})

llvm_map_components_to_libnames(LLVM_LIBS Core ExecutionEngine Interpreter Support native)

# ==========================================
# PHẦN 2: CẤU HÌNH DỰ ÁN MELLIS
# ==========================================

# Thư mục include chính
include_directories(include)

# ── Source files ──────────────────────────────────────────────────────────────
# Gom Frontend sources (parser frozen — no changes)
file(GLOB FRONTEND_SRC "src/FrontEnd/*.cpp")

# Gom AST sources
file(GLOB AST_SRC "src/AST/*.cpp")

# MiddleEnd sources — explicit list to exclude deprecated SemanticAnalyzer.cpp
set(MIDDLEEND_SRC
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/MiddleEnd/TypeChecker.cpp
    src/MiddleEnd/CFG.cpp
    src/MiddleEnd/MatchAnalyzer.cpp
    src/MiddleEnd/BorrowChecker.cpp
    src/MiddleEnd/MVIR.cpp
    src/MiddleEnd/MVIRGenerator.cpp
    src/MiddleEnd/Mangle.cpp
    src/MiddleEnd/SubstitutionVisitor.cpp
    src/MiddleEnd/MonomorphizationEngine.cpp
    src/MiddleEnd/Mangle.cpp
)

set(BACKEND_SRC
    src/BackEnd/LLVMIRGenerator.cpp
    src/BackEnd/ExecutableGenerator.cpp
)

set(SUPPORT_SRC
    src/Support/Diagnostic.cpp
    src/Support/LLDLinker.cpp
)

# Tạo file thực thi chính
add_executable(mellis
    src/main.cpp
    ${AST_SRC}
    ${FRONTEND_SRC}
    ${MIDDLEEND_SRC}
    ${BACKEND_SRC}
    ${SUPPORT_SRC}

    src/Core/SourceManager.cpp
    src/Core/CompilerSession.cpp
)

# Liên kết với LLVM
target_link_libraries(mellis PRIVATE ${LLVM_LIBS})

# Runtime Library
add_library(mellis_rt STATIC src/Runtime/runtime.c)
set_property(TARGET mellis_rt PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded")

# ==========================================
# PHẦN 3: KIỂM THỬ (TESTING)
# ==========================================
enable_testing()

# ── test_ast_manual ───────────────────────────────────────────────────────────
add_executable(test_ast_manual
    tests/test_ast_manual.cpp
    ${AST_SRC}
    ${SUPPORT_SRC}
    src/Core/SourceManager.cpp
)
target_include_directories(test_ast_manual PRIVATE include)
add_test(NAME ASTManualTest COMMAND test_ast_manual)

# ── test_lexer ────────────────────────────────────────────────────────────────
add_executable(test_lexer
    tests/test_lexer.cpp
    src/Core/SourceManager.cpp
    src/FrontEnd/Lexer.cpp
    src/Support/Diagnostic.cpp

    src/Core/SourceManager.cpp
)
target_include_directories(test_lexer PRIVATE include)
add_test(NAME LexerTest COMMAND test_lexer)

# ── test_parser ───────────────────────────────────────────────────────────────
add_executable(test_parser
    tests/test_parser.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_parser PRIVATE include)
add_test(NAME ParserTest COMMAND test_parser)

# ── test_resolver ─────────────────────────────────────────────────────────────
# Cần: Lexer, Parser, SymbolTable, Resolver
add_executable(test_resolver
    tests/test_resolver.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_resolver PRIVATE include)
add_test(NAME ResolverTest COMMAND test_resolver)

# ── test_diagnostic ───────────────────────────────────────────────────────────
# Cần: DiagnosticEngine only (no Lexer/Parser/Resolver)
add_executable(test_diagnostic
    tests/test_diagnostic.cpp
    src/Support/Diagnostic.cpp

    src/Core/SourceManager.cpp
)
target_include_directories(test_diagnostic PRIVATE include)
add_test(NAME DiagnosticTest COMMAND test_diagnostic)

# ── test_typechecker ──────────────────────────────────────────────────────────
add_executable(test_typechecker
    tests/test_typechecker.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/MiddleEnd/TypeChecker.cpp
    src/MiddleEnd/MatchAnalyzer.cpp
    src/MiddleEnd/MonomorphizationEngine.cpp
    src/MiddleEnd/Mangle.cpp
    src/MiddleEnd/SubstitutionVisitor.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_typechecker PRIVATE include)
add_test(NAME TypeCheckerTest COMMAND test_typechecker)

# ── test_cfg ──────────────────────────────────────────────────────────
add_executable(test_cfg
    tests/test_cfg.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/MiddleEnd/CFG.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_cfg PRIVATE include)
add_test(NAME CFGTest COMMAND test_cfg)

# ── test_mvir_generator ───────────────────────────────────────────────────────
add_executable(test_mvir_generator
    tests/test_mvir_generator.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/MiddleEnd/TypeChecker.cpp
    src/MiddleEnd/MVIR.cpp
    src/MiddleEnd/MVIRGenerator.cpp src/MiddleEnd/MatchAnalyzer.cpp src/MiddleEnd/BorrowChecker.cpp
    src/MiddleEnd/MonomorphizationEngine.cpp
    src/MiddleEnd/Mangle.cpp src/MiddleEnd/SubstitutionVisitor.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_mvir_generator PRIVATE include)
add_test(NAME MVIRGeneratorTest COMMAND test_mvir_generator)

# ── test_borrow_checker ───────────────────────────────────────────────────────
add_executable(test_borrow_checker
    tests/test_borrow_checker.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/MiddleEnd/TypeChecker.cpp
    src/MiddleEnd/MVIR.cpp
    src/MiddleEnd/MVIRGenerator.cpp src/MiddleEnd/MatchAnalyzer.cpp src/MiddleEnd/BorrowChecker.cpp
    src/MiddleEnd/BorrowChecker.cpp
    src/MiddleEnd/MonomorphizationEngine.cpp
    src/MiddleEnd/Mangle.cpp src/MiddleEnd/SubstitutionVisitor.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_borrow_checker PRIVATE include)
add_test(NAME BorrowCheckerTest COMMAND test_borrow_checker)

# ── test_llvmir_generator ───────────────────────────────────────────────────────
add_executable(test_llvmir_generator
    tests/test_llvmir_generator.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/MiddleEnd/TypeChecker.cpp
    src/MiddleEnd/MVIR.cpp
    src/MiddleEnd/MVIRGenerator.cpp src/MiddleEnd/MatchAnalyzer.cpp src/MiddleEnd/BorrowChecker.cpp
    src/MiddleEnd/MonomorphizationEngine.cpp
    src/MiddleEnd/Mangle.cpp src/MiddleEnd/SubstitutionVisitor.cpp
    src/BackEnd/LLVMIRGenerator.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_llvmir_generator PRIVATE include)
target_link_libraries(test_llvmir_generator PRIVATE ${LLVM_LIBS})
add_test(NAME LLVMIRGeneratorTest COMMAND test_llvmir_generator)

# ── test_executable_generator ───────────────────────────────────────────────────
add_executable(test_executable_generator
    tests/test_executable_generator.cpp
    src/FrontEnd/Lexer.cpp
    src/FrontEnd/Parser.cpp
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/MiddleEnd/TypeChecker.cpp
    src/MiddleEnd/MVIR.cpp
    src/MiddleEnd/MVIRGenerator.cpp src/MiddleEnd/MatchAnalyzer.cpp src/MiddleEnd/BorrowChecker.cpp
    src/MiddleEnd/MonomorphizationEngine.cpp
    src/MiddleEnd/Mangle.cpp src/MiddleEnd/SubstitutionVisitor.cpp
    src/BackEnd/LLVMIRGenerator.cpp
    src/BackEnd/ExecutableGenerator.cpp
    src/Support/LLDLinker.cpp
    src/Support/ClangLinker.cpp
    src/Support/Diagnostic.cpp
    ${AST_SRC}

    src/Core/SourceManager.cpp
)
target_include_directories(test_executable_generator PRIVATE include)
target_link_libraries(test_executable_generator PRIVATE ${LLVM_LIBS})
add_test(NAME ExecutableGeneratorTest COMMAND test_executable_generator)
