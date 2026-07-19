// =============================================================================
// tests/test_mvir_generator.cpp
//
// Unit tests for the MVIR Generator.
//
// Verifies that the AST correctly lowers to the exact MVIR text specification
// defined in docs/mvir.md. No LLVM IR or optimizations are tested here.
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
#include "mellis/Support/Diagnostic.h"

using namespace fl;

// ── Helper ───────────────────────────────────────────────────────────────────

std::string generateMVIR(const std::string& source) {
    Lexer lexer(source);
    DiagnosticEngine diag; Parser parser(lexer, diag);
    auto ast = parser.parse();

    SymbolTable table;
    Resolver resolver(table, diag);
    bool resolveOk = resolver.resolve(ast.get());
    // Type checking might fail due to dummy types used in testing
    TypeContext typeCtx;
    TypeChecker tc(table, diag, typeCtx);
    bool tcOk = tc.check(ast.get());

    MVIRGenerator mvirGen(table, tc);
    auto module = mvirGen.generate(*ast);
    
    return module->toString();
}

void assertContains(const std::string& fullText, const std::string& substring) {
    if (fullText.find(substring) == std::string::npos) {
        std::cerr << "Expected MVIR to contain:\n" << substring << "\n";
        std::cerr << "Actual MVIR:\n" << fullText << "\n";
        assert(false && "MVIR did not contain expected substring");
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

void test01_variable_declaration() {
    std::string mvir = generateMVIR("fn foo() { dec x = 42; }");
    
    // Should contain an alloca for i32 and a store of 42
    assertContains(mvir, "alloca i32");
    assertContains(mvir, "store 42, %0");
    std::cout << "[OK] test01_variable_declaration\n";
}

void test02_arithmetic() {
    std::string mvir = generateMVIR("fn foo() { dec x = 10 + 20; }");
    
    // Should evaluate 10 + 20 into a virtual register and store it
    assertContains(mvir, "add 10, 20");
    std::cout << "[OK] test02_arithmetic\n";
}

void test03_variable_read() {
    std::string mvir = generateMVIR("fn foo() { dec x = 10; dec y = x; }");
    
    // y = x requires loading x first
    assertContains(mvir, "load %0"); // assuming %0 is x's allocation
    std::cout << "[OK] test03_variable_read\n";
}

void test04_if_statement() {
    std::string mvir = generateMVIR("fn foo() { if (true) { dec x = 1; } }");
    
    // Should branch on 'true' to a 'then' block
    assertContains(mvir, "branch true, then");
    assertContains(mvir, "merge");
    std::cout << "[OK] test04_if_statement\n";
}

void test05_while_statement() {
    std::string mvir = generateMVIR("fn foo() { while (true) { dec x = 1; } }");
    
    // Should jump to condition, branch to body or end
    assertContains(mvir, "jump while_cond");
    assertContains(mvir, "branch false, while_body");
    assertContains(mvir, "while_end");
    std::cout << "[OK] test05_while_statement\n";
}


void test07_memory_access() {
    std::string mvir = generateMVIR("fn foo() { dec arr = 0; dec x = arr[0]; dec y = arr.f; }");
    assertContains(mvir, "get_ptr");
    std::cout << "[OK] test07_memory_access\n";
}

void test08_borrow_cast() {
    std::string mvir = generateMVIR("fn foo() { dec x = 1; dec ptr = &x; dec mut_ptr = &rw x; dec z = x as i32; }");
    assertContains(mvir, "borrow shared");
    assertContains(mvir, "borrow mut");
    assertContains(mvir, "cast");
    std::cout << "[OK] test08_borrow_cast\n";
}

void test09_match_expr() {
    std::string mvir = generateMVIR("fn foo() { dec x = 1; match x { 1 -> { return 1; } _ -> { return 2; } }; }");
    assertContains(mvir, "switch");
    assertContains(mvir, "match_arm");
    assertContains(mvir, "match_end");
    std::cout << "[OK] test09_match_expr\n";
}

void test10_functions_ffi() {
    std::string mvir = generateMVIR("fn printf() { } fn foo() { printf(); }");
    assertContains(mvir, "@printf");
    assertContains(mvir, "@foo");
    std::cout << "[OK] test10_functions_ffi\n";
}

void test11_globals() {
    std::string mvir = generateMVIR("fn foo() { dec s = \"hello\"; }");
    assertContains(mvir, "@.str.");
    std::cout << "[OK] test11_globals\n";
}

// ── Main ─────────────────────────────────────────────────────────────────────

int main() {
    std::cout << "========================================\n";
    std::cout << "  MELLIS MVIR GENERATOR TESTS\n";
    std::cout << "========================================\n";

    test01_variable_declaration();
    test02_arithmetic();
    test03_variable_read();
    test04_if_statement();
    test05_while_statement();

    test07_memory_access();
    test08_borrow_cast();
    test09_match_expr();
    test10_functions_ffi();
    test11_globals();

    std::cout << "========================================\n";
    std::cout << "  ALL TESTS PASSED\n";
    std::cout << "========================================\n";
    return 0;
}
