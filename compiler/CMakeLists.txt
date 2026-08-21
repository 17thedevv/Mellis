cmake_minimum_required(VERSION 3.20)
project(mellis-compiler VERSION 1.0.0 LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ==========================================
# LLVM
# ==========================================
list(APPEND CMAKE_PREFIX_PATH "D:/Programs/LLVM-DEV")
find_package(LLVM REQUIRED CONFIG)

include_directories(${LLVM_INCLUDE_DIRS})
link_directories(${LLVM_LIBRARY_DIRS})
link_directories("C:/Program Files/Microsoft Visual Studio/18/Community/DIA SDK/lib/amd64")
add_definitions(${LLVM_DEFINITIONS})

llvm_map_components_to_libnames(LLVM_LIBS Core ExecutionEngine Interpreter Support native Passes Coroutines)

# ==========================================
# Compiler include directories
# ==========================================
include_directories(include)

# ── Source files ──────────────────────────────────────────────────────────────
file(GLOB MLIB_SRC      "src/MLib/*.cpp")
file(GLOB LSP_SRC       "src/LSP/*.cpp")
file(GLOB FRONTEND_SRC  "src/FrontEnd/*.cpp")
file(GLOB AST_SRC       "src/AST/*.cpp")
file(GLOB CORE_SRC      "src/Core/*.cpp")

set(MIDDLEEND_SRC
    src/MiddleEnd/SymbolTable.cpp
    src/MiddleEnd/Resolver.cpp
    src/MiddleEnd/TypeChecker.cpp
    src/MiddleEnd/CFG.cpp
    src/MiddleEnd/MatchAnalyzer.cpp
    src/MiddleEnd/EscapeAnalyzer.cpp
    src/MiddleEnd/BorrowAnalyzer.cpp
    src/MiddleEnd/LivenessAnalyzer.cpp
    src/MiddleEnd/MoveAnalyzer.cpp
    src/MiddleEnd/InitializationAnalyzer.cpp
    src/MiddleEnd/LifetimeConstraintSolver.cpp
    src/MiddleEnd/Mangle.cpp
    src/MiddleEnd/SubstitutionVisitor.cpp
    src/MiddleEnd/MonomorphizationEngine.cpp
    src/MiddleEnd/ConstEvaluator.cpp
    src/MiddleEnd/ComptimeEvaluator.cpp
    src/MiddleEnd/SemanticAnalyzer.cpp
    src/MiddleEnd/TraitSolver.cpp
    src/MiddleEnd/MethodResolver.cpp
    src/MiddleEnd/VisibilityLeakChecker.cpp
    src/MiddleEnd/OperatorRegistry.cpp
)

set(IR_SRC
    src/IR/MVIR.cpp
    src/IR/MVIRGenerator.cpp
    src/IR/IRVerifier.cpp
)

set(OPTIMIZER_SRC
    src/Optimizer/MVIROptimizer.cpp
)

set(SEMANTIC_SRC
    src/MiddleEnd/Semantic/DropElaborator.cpp
    src/MiddleEnd/Semantic/ObjectSafety.cpp
)

set(ABI_SRC
    src/MiddleEnd/ABI/TraitObjectLayout.cpp
)

set(BACKEND_SRC
    src/BackEnd/LLVMIRGenerator.cpp
    src/BackEnd/ExecutableGenerator.cpp
)

set(SUPPORT_SRC
    src/Support/Diagnostic.cpp
    src/Support/OSUtils.cpp
    src/Support/LLDLinker.cpp
    src/Support/ClangLinker.cpp
)

add_executable(mellis
    src/main.cpp
    ${AST_SRC}
    ${FRONTEND_SRC}
    ${MIDDLEEND_SRC}
    ${SEMANTIC_SRC}
    ${ABI_SRC}
    ${IR_SRC}
    ${OPTIMIZER_SRC}
    ${BACKEND_SRC}
    ${SUPPORT_SRC}
    ${MLIB_SRC}
    ${LSP_SRC}
    ${CORE_SRC}
)

target_link_libraries(mellis PRIVATE ${LLVM_LIBS} -lmsvcrt -lvcruntime -lucrt)
