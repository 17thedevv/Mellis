// =============================================================================
// tests/test_llvmir_generator.cpp
//
// Unit tests for the LLVM IR Generator.
// =============================================================================

#include <cassert>
#include <iostream>
#include <sstream>
#include "mellis/FrontEnd/Lexer.h"
#include "mellis/FrontEnd/Parser.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/Resolver.h"
#include "mellis/MiddleEnd/TypeChecker.h"
#include "mellis/MiddleEnd/MVIRGenerator.h"
#include "mellis/BackEnd/LLVMIRGenerator.h"
#include <llvm/Support/raw_ostream.h>

using namespace fl;

// ── Helper ───────────────────────────────────────────────────────────────────

std::string generateLLVM(const std::string& source) {
    Lexer lexer(source);
    DiagnosticEngine diag; 
    Parser parser(lexer, diag);
    auto ast = parser.parse();

    SymbolTable table;
    Resolver resolver(table, diag);
    resolver.resolve(ast.get());

    TypeContext ctx;
    TypeChecker tc(table, diag, ctx);
    tc.check(ast.get());

    MVIRGenerator mvirGen(table, tc);
    auto* prog = dynamic_cast<ProgramNode*>(ast.get());
    auto mvirModule = mvirGen.generate(*prog);

    llvm::LLVMContext llvmCtx;
    llvm::Module llvmModule("test_module", llvmCtx);
    LLVMIRGenerator llvmGen(llvmCtx, llvmModule, table);
    bool ok = llvmGen.generate(mvirModule.get());
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
    std::string ir = generateLLVM("fn foo() { dec x = 42; }");
    
    assertContains(ir, "alloca i32");
    assertContains(ir, "store i32 42, ptr %");
    std::cout << "[OK] test01_variable_declaration\n";
}

void test02_arithmetic() {
    // We use variables to avoid llvm::IRBuilder's implicit constant folding
    std::string ir = generateLLVM("fn foo() { dec x = 10; dec y = x + 20; }");
    
    assertContains(ir, "add i32");
    std::cout << "[OK] test02_arithmetic\n";
}

void test03_if_statement() {
    std::string ir = generateLLVM("fn foo() { if (true) { dec x = 1; } }");
    
    assertContains(ir, "br i1 true");
    std::cout << "[OK] test03_if_statement\n";
}



// ── Main ─────────────────────────────────────────────────────────────────────

int main() {
    std::cout << "========================================\n";
    std::cout << "  MELLIS LLVM IR GENERATOR TESTS\n";
    std::cout << "========================================\n";

    test01_variable_declaration();
    test02_arithmetic();
    test03_if_statement();


    std::cout << "========================================\n";
    std::cout << "  ALL TESTS PASSED\n";
    std::cout << "========================================\n";
    return 0;
}
