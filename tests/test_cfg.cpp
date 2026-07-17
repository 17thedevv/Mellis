#include "fdlang/MiddleEnd/CFG.h"
#include "fdlang/FrontEnd/Lexer.h"
#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/AST/ProgramNode.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/Support/Diagnostic.h"
#include <iostream>

using namespace fl;

struct TestResult {
    bool ok;
    std::string err;
};

TestResult runCFG(const std::string& code, bool expectError = false) {
    DiagnosticEngine diag;
    Lexer lexer(code);
    Parser parser(lexer, diag);
    auto program = parser.parse();
    if (diag.hasErrors()) {
        if (expectError) return {true, ""};
        return {false, "Parser failed"};
    }
    
    ControlFlowAnalyzer analyzer(diag);
    
    bool allValid = true;
    for (auto& item : program->items) {
        if (auto* func = dynamic_cast<FunctionDeclNode*>(item.get())) {
            if (!analyzer.analyze(func)) {
                allValid = false;
            }
        }
    }
    
    if (expectError) {
        if (!diag.hasErrors()) {
            return {false, "Expected error, but CFG analyzer succeeded"};
        }
        return {true, ""};
    } else {
        if (diag.hasErrors()) {
            return {false, "CFG analyzer failed"};
        }
        return {true, ""};
    }
}

void test_cfg_valid_return() {
    auto r = runCFG(R"(
        fn main() -> int_32 {
            return 0;
        }
    )");
    if (!r.ok) { std::cout << "[FAIL] test_cfg_valid_return: " << r.err << "\n"; exit(1); }
    std::cout << "[OK] test_cfg_valid_return\n";
}

void test_cfg_missing_return() {
    auto r = runCFG(R"(
        fn main() -> int_32 {
            dec x = 10;
        }
    )", true);
    if (!r.ok) { std::cout << "[FAIL] test_cfg_missing_return: " << r.err << "\n"; exit(1); }
    std::cout << "[OK] test_cfg_missing_return\n";
}

void test_cfg_if_else_return() {
    auto r = runCFG(R"(
        fn check(x: int_32) -> int_32 {
            if x == 10 {
                return 1;
            } else {
                return 2;
            }
        }
    )");
    if (!r.ok) { std::cout << "[FAIL] test_cfg_if_else_return: " << r.err << "\n"; exit(1); }
    std::cout << "[OK] test_cfg_if_else_return\n";
}

void test_cfg_if_else_missing_return() {
    auto r = runCFG(R"(
        fn check(x: int_32) -> int_32 {
            if x == 10 {
                return 1;
            } else {
                dec y = 2;
            }
        }
    )", true);
    if (!r.ok) { std::cout << "[FAIL] test_cfg_if_else_missing_return: " << r.err << "\n"; exit(1); }
    std::cout << "[OK] test_cfg_if_else_missing_return\n";
}

void test_cfg_break_outside_loop() {
    auto r = runCFG(R"(
        fn check() {
            break;
        }
    )", true);
    if (!r.ok) { std::cout << "[FAIL] test_cfg_break_outside_loop: " << r.err << "\n"; exit(1); }
    std::cout << "[OK] test_cfg_break_outside_loop\n";
}

void test_cfg_break_inside_loop() {
    auto r = runCFG(R"(
        fn check() {
            while 1 == 1 {
                break;
            }
        }
    )");
    if (!r.ok) { std::cout << "[FAIL] test_cfg_break_inside_loop: " << r.err << "\n"; exit(1); }
    std::cout << "[OK] test_cfg_break_inside_loop\n";
}

int main() {
    std::cout << "========================================\n";
    std::cout << "  FDLANG CFG TESTS\n";
    std::cout << "========================================\n";
    
    test_cfg_valid_return();
    test_cfg_missing_return();
    test_cfg_if_else_return();
    test_cfg_if_else_missing_return();
    test_cfg_break_outside_loop();
    test_cfg_break_inside_loop();
    
    std::cout << "============================\n";
    std::cout << "All CFG tests passed!\n";
    std::cout << "============================\n";
    return 0;
}
