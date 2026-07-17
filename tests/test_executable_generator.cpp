// =============================================================================
// tests/test_executable_generator.cpp
//
// Integration test: AST -> FLIR -> LLVM IR -> Executable -> Execution.
// =============================================================================

#include <cassert>
#include <iostream>
#include <fstream>
#include <cstdlib>
#include "fdlang/FrontEnd/Lexer.h"
#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/MiddleEnd/SymbolTable.h"
#include "fdlang/MiddleEnd/Resolver.h"
#include "fdlang/MiddleEnd/TypeChecker.h"
#include "fdlang/MiddleEnd/FLIRGenerator.h"
#include "fdlang/BackEnd/LLVMIRGenerator.h"
#include "fdlang/BackEnd/ExecutableGenerator.h"
#include "fdlang/Support/ClangLinker.h"
#include <llvm/Support/FileSystem.h>

using namespace fl;

void test_end_to_end_execution() {
    std::string source = "dec x = 10; print x;";
    
    Lexer lexer(source);
    DiagnosticEngine diag; Parser parser(lexer, diag);
    auto ast = parser.parse();

    DiagnosticEngine diag;
    SymbolTable table;
    Resolver resolver(table, diag);
    resolver.resolve(ast.get());

    TypeChecker tc(table, diag);
    tc.check(ast.get());

    FLIRGenerator flirGen(table, tc);
    auto flirModule = flirGen.generate(ast.get());

    llvm::LLVMContext ctx;
    llvm::Module llvmModule("test_module", ctx);
    LLVMIRGenerator llvmGen(ctx, llvmModule);
    bool ok = llvmGen.generate(flirModule.get());
    if (!ok) {
        std::cerr << "LLVM generation failed\n";
        std::exit(1);
    }

    int clang_res = std::system("clang --version > NUL 2>&1");
    if (clang_res != 0) {
        std::cout << "[SKIP] clang not found in PATH, skipping link and execution phase.\n";
        
        // Still verify object emission using a mock linker
        class MockLinker : public Linker {
        public:
            bool link(const std::string& obj, const std::string& out, const std::vector<std::string>&) override {
                std::cout << "Mock Linker called with obj: " << obj << "\n";
                return true;
            }
        };
        
        MockLinker mock;
        ExecutableGenerator exeGenMock(diag, mock);
        std::string outExe = ".\\test_e2e_output.exe";
        bool exeOk = exeGenMock.generateExecutable(&llvmModule, outExe);
        if (!exeOk) {
            std::cerr << "Mock Executable generation failed\n";
            std::exit(1);
        }
        std::cout << "[OK] test_end_to_end_execution (Mocked Linker)\n";
        llvm::sys::fs::remove("test_e2e_output.obj");
        return;
    }

    ClangLinker linker(diag);
    ExecutableGenerator exeGen(diag, linker);
    
    std::string outExe = ".\\test_e2e_output.exe";
    bool exeOk = exeGen.generateExecutable(&llvmModule, outExe);
    if (!exeOk) {
        std::cerr << "Executable generation failed\n";
        std::exit(1);
    }

    // Run the executable and redirect output
    int res = std::system((outExe + " > test_e2e_result.txt").c_str());
    if (res != 0) {
        std::cerr << "Execution of generated binary failed\n";
        std::exit(1);
    }

    std::ifstream inFile("test_e2e_result.txt");
    std::string output;
    inFile >> output;
    if (output != "10") {
        std::cerr << "Expected output 10, got: " << output << "\n";
        std::exit(1);
    }

    std::cout << "[OK] test_end_to_end_execution\n";

    // Cleanup
    llvm::sys::fs::remove(outExe);
    llvm::sys::fs::remove("test_e2e_output.obj");
    llvm::sys::fs::remove("test_e2e_result.txt");
}

int main() {
    std::cout << "========================================\n";
    std::cout << "  FDLANG EXECUTABLE GENERATOR TESTS\n";
    std::cout << "========================================\n";

    test_end_to_end_execution();

    std::cout << "========================================\n";
    std::cout << "  ALL TESTS PASSED\n";
    std::cout << "========================================\n";
    return 0;
}
