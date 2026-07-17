// =============================================================================
// tests/test_llvmir_generator.cpp
//
// Unit tests for the LLVM IR Generator.
// =============================================================================

#include <cassert>
#include <iostream>
#include <sstream>
#include "fdlang/FrontEnd/Lexer.h"
#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/MiddleEnd/SymbolTable.h"
#include "fdlang/MiddleEnd/Resolver.h"
#include "fdlang/MiddleEnd/TypeChecker.h"
#include "fdlang/MiddleEnd/FLIRGenerator.h"
#include "fdlang/BackEnd/LLVMIRGenerator.h"
#include <llvm/Support/raw_ostream.h>

using namespace fl;

// ── Helper ───────────────────────────────────────────────────────────────────

std::string generateLLVM(const std::string& source) {
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
    assert(ok && "LLVM generation/verification failed");

    std::string outStr;
    llvm::raw_string_ostream os(outStr);
    llvmModule.print(os, nullptr);
    os.flush();
    return outStr;
}

void assertContains(const std::string& fullText, const std::string& substring) {
    if (fullText.find(substring) == std::string::npos) {
        std::cerr << "Expected LLVM IR to contain:\n" << substring << "\n";
        std::cerr << "Actual LLVM IR:\n" << fullText << "\n";
        std::exit(1);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

void test01_variable_declaration() {
    std::string ir = generateLLVM("dec x = 42;");
    
    assertContains(ir, "alloca i32");
    assertContains(ir, "store i32 42, ptr %");
    std::cout << "[OK] test01_variable_declaration\n";
}

void test02_arithmetic() {
    // We use variables to avoid llvm::IRBuilder's implicit constant folding
    std::string ir = generateLLVM("dec x = 10; dec y = x + 20;");
    
    assertContains(ir, "add i32");
    std::cout << "[OK] test02_arithmetic\n";
}

void test03_if_statement() {
    std::string ir = generateLLVM("if (true) { dec x = 1; }");
    
    assertContains(ir, "br i1 true");
    std::cout << "[OK] test03_if_statement\n";
}

void test04_print_call() {
    std::string ir = generateLLVM("print 100;");
    
    assertContains(ir, "declare void @print(i32)");
    assertContains(ir, "call void @print(i32 100)");
    std::cout << "[OK] test04_print_call\n";
}

// ── Main ─────────────────────────────────────────────────────────────────────

int main() {
    std::cout << "========================================\n";
    std::cout << "  FDLANG LLVM IR GENERATOR TESTS\n";
    std::cout << "========================================\n";

    test01_variable_declaration();
    test02_arithmetic();
    test03_if_statement();
    test04_print_call();

    std::cout << "========================================\n";
    std::cout << "  ALL TESTS PASSED\n";
    std::cout << "========================================\n";
    return 0;
}
