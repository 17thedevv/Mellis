// =============================================================================
// tests/test_flir_generator.cpp
//
// Unit tests for the FLIR Generator.
//
// Verifies that the AST correctly lowers to the exact FLIR text specification
// defined in docs/flir.md. No LLVM IR or optimizations are tested here.
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

using namespace fl;

// ── Helper ───────────────────────────────────────────────────────────────────

std::string generateFLIR(const std::string& source) {
    Lexer lexer(source);
    DiagnosticEngine diag; Parser parser(lexer, diag);
    auto ast = parser.parse();

    DiagnosticEngine diag;
    SymbolTable table;
    Resolver resolver(table, diag);
    bool resolveOk = resolver.resolve(ast.get());
    assert(resolveOk && "Resolver failed in FLIR test");

    TypeChecker tc(table, diag);
    bool tcOk = tc.check(ast.get());
    assert(tcOk && "TypeChecker failed in FLIR test");

    FLIRGenerator flirGen(table, tc);
    auto module = flirGen.generate(ast.get());
    
    return module->toString();
}

void assertContains(const std::string& fullText, const std::string& substring) {
    if (fullText.find(substring) == std::string::npos) {
        std::cerr << "Expected FLIR to contain:\n" << substring << "\n";
        std::cerr << "Actual FLIR:\n" << fullText << "\n";
        assert(false && "FLIR did not contain expected substring");
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

void test01_variable_declaration() {
    std::string flir = generateFLIR("dec x = 42;");
    
    // Should contain an alloca for i32 and a store of 42
    assertContains(flir, "alloca i32");
    assertContains(flir, "store 42, %0");
    std::cout << "[OK] test01_variable_declaration\n";
}

void test02_arithmetic() {
    std::string flir = generateFLIR("dec x = 10 + 20;");
    
    // Should evaluate 10 + 20 into a virtual register and store it
    assertContains(flir, "add 10, 20");
    std::cout << "[OK] test02_arithmetic\n";
}

void test03_variable_read() {
    std::string flir = generateFLIR("dec x = 1; dec y = x;");
    
    // y = x requires loading x first
    assertContains(flir, "load %0"); // assuming %0 is x's allocation
    std::cout << "[OK] test03_variable_read\n";
}

void test04_if_statement() {
    std::string flir = generateFLIR("if (true) { dec x = 1; }");
    
    // Should branch on 'true' to a 'then' block
    assertContains(flir, "branch true, then");
    assertContains(flir, "merge");
    std::cout << "[OK] test04_if_statement\n";
}

void test05_while_statement() {
    std::string flir = generateFLIR("while (false) { dec x = 1; }");
    
    // Should jump to condition, branch to body or end
    assertContains(flir, "jump while_cond");
    assertContains(flir, "branch false, while_body");
    assertContains(flir, "while_end");
    std::cout << "[OK] test05_while_statement\n";
}

void test06_print_call() {
    std::string flir = generateFLIR("print 100;");
    
    // print should lower to a call instruction
    assertContains(flir, "call @print(100)");
    std::cout << "[OK] test06_print_call\n";
}

// ── Main ─────────────────────────────────────────────────────────────────────

int main() {
    std::cout << "========================================\n";
    std::cout << "  FDLANG FLIR GENERATOR TESTS\n";
    std::cout << "========================================\n";

    test01_variable_declaration();
    test02_arithmetic();
    test03_variable_read();
    test04_if_statement();
    test05_while_statement();
    test06_print_call();

    std::cout << "========================================\n";
    std::cout << "  ALL TESTS PASSED\n";
    std::cout << "========================================\n";
    return 0;
}
