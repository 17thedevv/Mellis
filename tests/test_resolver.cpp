// =============================================================================
// tests/test_resolver.cpp
//
// Unit tests for the Resolver subsystem (Multi-Pass Architecture).
// =============================================================================

#include <cassert>
#include <iostream>
#include <sstream>
#include "fdlang/FrontEnd/Lexer.h"
#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/MiddleEnd/SymbolTable.h"
#include "fdlang/MiddleEnd/Resolver.h"
#include "fdlang/Support/Diagnostic.h"

using namespace fl;

struct ResolveResult {
    bool        ok;
    size_t      errorCount;
    size_t      symbolCount;
    size_t      scopeCount;
};

ResolveResult runResolver(const std::string& source, bool suppressErrors = false) {
    Lexer  lexer(source);
    DiagnosticEngine diag; Parser parser(lexer, diag);
    auto   ast = parser.parse();
    if (diag.hasErrors()) {
        if (!suppressErrors) diag.flush();
        return {false, diag.errorCount(), 0, 0};
    }

    DiagnosticEngine resDiag;
    SymbolTable      table;
    Resolver         resolver(table, resDiag);
    bool ok = resolver.resolve(ast.get());
    
    if (!ok && !suppressErrors) {
        resDiag.flush();
    }

    return {ok, resDiag.errorCount(), table.symbolCount(), table.scopeCount()};
}

void test01_simpleDeclarationAndUse() {
    auto r = runResolver(R"(
        fn main() -> void {
            dec x = 10;
            x = x + 1;
        }
    )");

    if (!r.ok) {
        std::cout << "[FAIL] test01_simpleDeclarationAndUse\\n";
        exit(1);
    }
    std::cout << "[OK] test01_simpleDeclarationAndUse\\n";
}

void test02_useBeforeDeclare_AllowedInTopLevel() {
    // In multi-pass, you CAN call a function before it is declared if it's top-level
    auto r = runResolver(R"(
        fn main() -> void {
            foo();
        }
        fn foo() -> void {}
    )");

    if (!r.ok) {
        std::cout << "[FAIL] test02_useBeforeDeclare_AllowedInTopLevel\\n";
        exit(1);
    }
    std::cout << "[OK] test02_useBeforeDeclare_AllowedInTopLevel\\n";
}

void test03_undeclaredIdentifier() {
    auto r = runResolver(R"(
        fn main() -> void {
            dec x = z;
        }
    )", true);

    if (r.ok) {
        std::cout << "[FAIL] test03_undeclaredVariable_ShouldFail\\n";
        exit(1);
    }
    std::cout << "[OK] test03_undeclaredVariable_ShouldFail\\n";
}

void test04_shadowingInNestedScope() {
    auto r = runResolver(R"(
        fn main() -> void {
            dec x = 1;
            {
                dec x = 2;
                x = 3;
            }
            x = 4;
        }
    )");

    if (!r.ok) {
        std::cout << "[FAIL] test04_shadowingInNestedScope\\n";
        exit(1);
    }
    std::cout << "[OK] test04_shadowingInNestedScope\\n";
}

void test05_structAndFields() {
    auto r = runResolver(R"(
        struct Point {
            x: int_32,
            y: int_32
        }
        
        fn main() -> void {
            dec p = Point { x: 1, y: 2 };
        }
    )");

    if (!r.ok) {
        std::cout << "[FAIL] test05_basicStructsAndFunctions\\n";
        exit(1);
    }
    std::cout << "[OK] test05_basicStructsAndFunctions\\n";
}

void test06_generics() {
    auto r = runResolver(R"(
        fn id<T>(x: T) -> T {
            return x;
        }
        struct Option<T> {
            val: T
        }
    )");

    if (!r.ok) {
        std::cout << "[FAIL] test06_generics\\n";
        exit(1);
    }
    std::cout << "[OK] test06_generics\\n";
}

void test07_patternBindings() {
    // Tests for in arr syntax is not fully supported by grammar unless it's an expression.
    // We will use match arm binding or simple local destructure if supported.
    // Since MatchArm parsing is supported: match x { Foo => 1 }
    // Wait, the parser supports identifier patterns!
    auto r = runResolver(R"(
        fn main() -> void {
            dec x = 10;
            match x {
                some_var -> {
                    some_var = 20; // some_var should be bound in the arm scope
                }
            };
        }
    )");

    if (!r.ok) {
        std::cout << "[FAIL] test07_patternBindings\\n";
        exit(1);
    }
    std::cout << "[OK] test07_patternBindings\\n";
}

int main() {
    std::cout << "========================================\n";
    std::cout << "  FDLANG RESOLVER TESTS\n";
    std::cout << "========================================\n";

    test01_simpleDeclarationAndUse();
    test02_useBeforeDeclare_AllowedInTopLevel();
    test03_undeclaredIdentifier();
    test04_shadowingInNestedScope();
    test05_structAndFields();
    test06_generics();
    test07_patternBindings();

    std::cout << "============================\\n";
    std::cout << "All resolver tests passed!\\n";
    std::cout << "============================\\n";
    return 0;
}
