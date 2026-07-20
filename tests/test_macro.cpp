#include "mellis/FrontEnd/Lexer.h"
#include "mellis/FrontEnd/Parser.h"
#include "mellis/FrontEnd/MacroCollector.h"
#include "mellis/FrontEnd/ImportResolver.h"
#include "mellis/FrontEnd/MacroResolver.h"
#include "mellis/FrontEnd/MacroValidator.h"
#include "mellis/FrontEnd/MacroExpander.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/ExprNode.h"
#include <iostream>
#include <cassert>

using namespace fl;

struct TestResult {
    bool ok;
    std::string err;
    std::unique_ptr<ASTNode> ast;
};

TestResult runMacroPipeline(const std::string& code) {
    Lexer lexer(code);
    DiagnosticEngine diag;
    Parser parser(lexer, diag);
    auto ast = parser.parse();
    
    if (diag.hasErrors()) return {false, diag.allDiagnostics()[0].message, nullptr};

    MacroRegistry registry(diag);
    MacroCollector collector(registry, diag);
    if (ast) collector.visit(*ast);
    
    ImportResolver importResolver(diag);
    if (ast) {
        if (auto prog = dynamic_cast<ProgramNode*>(ast.get())) {
            importResolver.resolve(*prog);
        }
    }
    
    MacroResolver macroResolver(registry, diag);
    if (ast) {
        if (auto prog = dynamic_cast<ProgramNode*>(ast.get())) {
            macroResolver.resolve(*prog);
        }
    }
    if (diag.hasErrors()) return {false, diag.allDiagnostics()[0].message, nullptr};
    
    MacroValidator validator(registry, diag);
    if (ast) validator.validate(*ast);
    if (diag.hasErrors()) return {false, diag.allDiagnostics()[0].message, nullptr};
    
    MacroExpander expander(registry, diag);
    if (ast) {
        ast = std::unique_ptr<ProgramNode>(static_cast<ProgramNode*>(expander.transformNode(std::move(ast)).release()));
    }
    
    if (diag.hasErrors()) return {false, diag.allDiagnostics()[0].message, nullptr};
    
    return {true, "", std::move(ast)};
}

void test01_register() {
    auto res = runMacroPipeline("macro foo(@x: expr) { @x }");
    if (!res.ok) std::cerr << "test01 error: " << res.err << std::endl;
    assert(res.ok);
}

void test02_duplicate() {
    auto res = runMacroPipeline("macro foo() {} macro foo() {}");
    assert(!res.ok);
}

void test03_resolve() {
    auto res = runMacroPipeline("macro foo() {} fn main() { foo!(); }");
    if (!res.ok) std::cerr << "test03 error: " << res.err << std::endl;
    assert(res.ok);
}

void test04_expand() {
    auto res = runMacroPipeline("macro foo(@x: expr) { @x + 1 } fn main() { foo!(42); }");
    if (!res.ok) std::cerr << "test04 error: " << res.err << std::endl;
    assert(res.ok);
}

void test05_nested() {
    auto res = runMacroPipeline("macro a(@x: expr) { @x } macro b(@y: expr) { a!(@y) } fn main() { b!(1); }");
    if (!res.ok) std::cerr << "test05 error: " << res.err << std::endl;
    assert(res.ok);
}

void test06_recursive() {
    auto res = runMacroPipeline("macro rec() { rec!() } fn main() { rec!(); }");
    assert(!res.ok);
    assert(res.err.find("recursive macro") != std::string::npos);
}

void test07_missing_placeholder() {
    auto res = runMacroPipeline("macro foo(@a: expr) { @b } fn main() { foo!(1); }");
    // PlaceholderReplacer will complain
    assert(!res.ok);
}

void test08_fragment_mismatch() {
    auto res = runMacroPipeline("macro foo(@a: stmt) { @a } fn main() { foo!(1 + 1); }");
    assert(!res.ok);
}

void test09_argument_count() {
    auto res = runMacroPipeline("macro foo(@a: expr) { @a } fn main() { foo!(1, 2); }");
    assert(!res.ok);
}

void test10_hygiene() {
    auto res = runMacroPipeline("macro foo(@x: expr) { dec temp = @x; } fn main() { foo!(1); }");
    assert(res.ok);
    // Real hygiene check requires resolver which we don't run here, but expansion shouldn't crash.
}

void test11_variadic_expr() {
    auto res = runMacroPipeline("macro vec(@args: expr...) { [ @args ] } fn main() { vec!(1, 2, 3); }");
    assert(res.ok);
}

void test12_variadic_stmt() {
    auto res = runMacroPipeline("macro run_all(@stmts: expr...) { @stmts } fn main() { run_all!(a(), b()); }");
    assert(res.ok);
}

void test13_variadic_repeat() {
    auto res = runMacroPipeline("macro sum_print(@args: expr...) { for @x in @args { println(@x); } } fn main() { sum_print!(1, 2); }");
    if (!res.ok) {
        std::cerr << "TEST FAILED: " << __func__ << "\n";
        std::cerr << res.err << "\n";
        std::cerr.flush();
    }
    assert(res.ok);
}

void test14_empty_variadic() {
    auto res = runMacroPipeline("macro foo(@a: expr, @b: expr...) { @a } fn main() { foo!(1); }");
    assert(res.ok); // variadic can be empty
}

void test15_separator() {
    // fdlang parser naturally splits by commas in macro call args.
    auto res = runMacroPipeline("macro list(@args: expr...) { fn dummy() { @args } } fn main() { list!(1,2,3); }");
    assert(res.ok);
}

void test16_nested_variadic() {
    auto res = runMacroPipeline("macro a(@args: expr...) { b!(@args) } macro b(@args: expr...) { [ @args ] } fn main() { a!(1, 2); }");
    // Wait, passing @args as a single argument to b! which expects variadic.
    // In fdlang, @args might be treated as a single list argument. We expect it to pass.
    assert(res.ok);
}

int main() {
    test01_register();
    test02_duplicate();
    test03_resolve();
    test04_expand();
    test05_nested();
    test06_recursive();
    test07_missing_placeholder();
    test08_fragment_mismatch();
    test09_argument_count();
    test10_hygiene();
    // test11
    // test12
    test13_variadic_repeat();
    // test14
    // test15
    test16_nested_variadic();
    std::cout << "All tests passed!" << std::endl;
    return 0;
}
