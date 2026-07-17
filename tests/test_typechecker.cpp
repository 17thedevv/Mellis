#include "fdlang/MiddleEnd/TypeChecker.h"
#include "fdlang/MiddleEnd/MatchAnalyzer.h"
#include "fdlang/MiddleEnd/Resolver.h"
#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/FrontEnd/Lexer.h"
#include <iostream>
#include <cstdlib>

using namespace fl;

struct TestResult {
    bool ok;
    std::string err;
};

TestResult runTypeChecker(const std::string& code, bool expectError = false) {
    Lexer lexer(code);
    SymbolTable table;
    DiagnosticEngine diag;
    TypeContext ctx;
    
    Parser parser(lexer, diag);
    auto root = parser.parse();
    
    Resolver resolver(table, diag);
    bool resolveOk = resolver.resolve(root.get());
    if (!resolveOk) {
        if (expectError) return {true, "resolver failed as expected"};
        return {false, "resolver failed: " + (diag.allDiagnostics().empty() ? "unknown" : diag.allDiagnostics()[0].message)};
    }
    
    TypeChecker checker(table, diag, ctx);
    bool typeOk = checker.check(root.get());
    

    if (!expectError && !typeOk) {
        return {false, "typecheck failed: " + (diag.allDiagnostics().empty() ? "unknown" : diag.allDiagnostics()[0].message)};
    }
    
    if (typeOk) {
        MatchAnalyzer matchAnalyzer(table, diag);
        matchAnalyzer.analyze(root.get());
        
        
        if (expectError && diag.errorCount() == 0) {
            return {false, "expected error, but succeeded"};
        }
        if (!expectError && diag.errorCount() > 0) {
            return {false, "analysis failed: " + diag.allDiagnostics().back().message};
        }
    }
    
    if (expectError && diag.errorCount() == 0) {
        return {false, "expected error, but succeeded"};
    }
    
    return {true, ""};
}

void test01_primitives() {
    auto r = runTypeChecker(R"(
        fn main() {
            dec x = 10;
            dec y = 3.14;
            dec z = true;
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test01_primitives: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test01_primitives\n";
}

void test02_binary_matching() {
    auto r = runTypeChecker(R"(
        fn main() {
            dec a = 10;
            dec b = 20;
            dec c = a + b;
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test02_binary_matching: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test02_binary_matching\n";
}

void test03_binary_mismatch() {
    auto r = runTypeChecker(R"(
        fn main() {
            dec a = 10;
            dec b = 3.14;
            dec c = a + b; // should fail
        }
    )", true);
    if (!r.ok) {
        std::cout << "[FAIL] test03_binary_mismatch (should fail): " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test03_binary_mismatch\n";
}

void test04_assign_mismatch() {
    auto r = runTypeChecker(R"(
        fn main() {
            dec a: int_32 = 3.14; // mismatch!
        }
    )", true);
    if (!r.ok) {
        std::cout << "[FAIL] test04_assign_mismatch (should fail): " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test04_assign_mismatch\n";
}

void test05_function_return() {
    auto r = runTypeChecker(R"(
        fn get_int() -> int_32 {
            return 10;
        }
        
        fn get_bool() -> bool {
            return 10; // mismatch!
        }
    )", true);
    if (!r.ok) {
        std::cout << "[FAIL] test05_function_return (should fail): " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test05_function_return\n";
}

void test06_struct_member() {
    auto r = runTypeChecker(R"(
        struct Point {
            x: int_32,
            y: int_32
        }
        
        fn main() {
            dec p = Point { x: 10, y: 20 };
            dec z = p.x + p.y;
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test06_struct_member: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test06_struct_member\n";
}

void test13_match_expr_exhaustive() {
    auto r = runTypeChecker(R"(
        enum Color { Red, Green, Blue }
        fn main() {
            dec c = Color::Red;
            dec x = match c {
                Color::Red -> 1,
                Color::Green -> 2,
                Color::Blue -> 3,
            };
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test13_match_expr_exhaustive: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test13_match_expr_exhaustive\n";
}

void test14_match_expr_non_exhaustive() {
    auto r = runTypeChecker(R"(
        enum Color { Red, Green, Blue }
        fn main() {
            dec c = Color::Red;
            dec x = match c {
                Color::Red -> 1,
                Color::Green -> 2,
                // Missing Blue
            };
        }
    )", true);
    if (!r.ok) {
        std::cout << "[FAIL] test14_match_expr_non_exhaustive (should fail): " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test14_match_expr_non_exhaustive\n";
}

void test15_match_expr_wildcard() {
    auto r = runTypeChecker(R"(
        enum Color { Red, Green, Blue }
        fn main() {
            dec c = Color::Red;
            dec x = match c {
                Color::Red -> 1,
                _ -> 2, // Catch-all covers Green and Blue
            };
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test15_match_expr_wildcard: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test15_match_expr_wildcard\n";
}

void test16_match_expr_return_mismatch() {
    auto r = runTypeChecker(R"(
        enum Color { Red, Green, Blue }
        fn main() {
            dec c = Color::Red;
            dec x = match c {
                Color::Red -> 1,
                Color::Green -> 3.14, // mismatch
                Color::Blue -> 3,
            };
        }
    )", true);
    if (!r.ok) {
        std::cout << "[FAIL] test16_match_expr_return_mismatch (should fail): " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test16_match_expr_return_mismatch\n";
}

void test17_named_arguments_valid() {
    auto r = runTypeChecker(R"(
        fn foo(x: int_32, y: int_32) -> int_32 { return x + y; }
        fn main() {
            dec a = foo(1, y: 2);
            dec b = foo(y: 20, x: 10);
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test17_named_arguments_valid: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test17_named_arguments_valid\n";
}

void test18_named_arguments_invalid() {
    auto r = runTypeChecker(R"(
        fn foo(x: int_32, y: int_32) -> int_32 { return x + y; }
        fn main() {
            // Positional after named is invalid
            dec a = foo(x: 1, 2);
        }
    )", true);
    if (!r.ok) {
        std::cout << "[FAIL] test18_named_arguments_invalid (should fail): " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test18_named_arguments_invalid\n";
}

void test19_method_call_valid() {
    auto r = runTypeChecker(R"(
        struct Point { x: int_32, y: int_32 }
        impl Point {
            fn distance(self: Point, other: Point) -> int_32 { return 0; }
        }
        fn main() {
            dec p1 = Point { x: 0, y: 0 };
            dec p2 = Point { x: 1, y: 1 };
            dec d = p1.distance(p2);
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test19_method_call_valid: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test19_method_call_valid\n";
}

void test20_method_call_invalid() {
    auto r = runTypeChecker(R"(
        struct Point { x: int_32, y: int_32 }
        impl Point {
            fn distance(self: Point, other: Point) -> int_32 { return 0; }
        }
        fn main() {
            dec p1 = Point { x: 0, y: 0 };
            dec d = p1.distance(10); // Error: expected Point, found int_32
        }
    )", true);
    if (!r.ok) {
        std::cout << "[FAIL] test20_method_call_invalid (should fail): " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test20_method_call_invalid\n";
}

void test21_unary_deref() {
    auto r = runTypeChecker(R"(
        fn main() {
            dec a: int_32 = 10;
            dec ptr: &rw int_32 = &rw a;
            dec val: int_32 = *ptr;
        }
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test21_unary_deref: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test21_unary_deref\n";
}

void test22_array_index() {
    auto r = runTypeChecker(R"(
        fn process_array(arr: [int_32, 5]) {
            dec val: int_32 = arr[0];
        }
        fn main() {}
    )");
    if (!r.ok) {
        std::cout << "[FAIL] test22_array_index: " << r.err << "\n";
        exit(1);
    }
    std::cout << "[OK] test22_array_index\n";
}

int main() {
    std::cout << "========================================\n";
    std::cout << "  FDLANG TYPECHECKER TESTS\n";
    std::cout << "========================================\n";
    test01_primitives();
    test02_binary_matching();
    test03_binary_mismatch();
    test04_assign_mismatch();
    test05_function_return();
    test06_struct_member();
    test13_match_expr_exhaustive();
    test14_match_expr_non_exhaustive();
    test15_match_expr_wildcard();
    test16_match_expr_return_mismatch();
    test17_named_arguments_valid();
    test18_named_arguments_invalid();
    test19_method_call_valid();
    test20_method_call_invalid();
    test21_unary_deref();
    test22_array_index();

    std::cout << "============================\n";
    std::cout << "All typechecker tests passed!\n";
    std::cout << "============================\n";
    return 0;
}
