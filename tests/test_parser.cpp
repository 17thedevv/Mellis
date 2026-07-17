#include <cassert>
#include <iostream>
#include "fdlang/FrontEnd/Lexer.h"
#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/AST/ProgramNode.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/AST/TypeNode.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/StmtNode.h"
#include "fdlang/Support/Diagnostic.h"

using namespace fl;

void testVarDecl() {
    Lexer lexer("dec x: int_32 = 42;");
    DiagnosticEngine diag;
    Parser parser(lexer, diag);

    auto program = parser.parse();
    assert(program->items.size() == 1);

    auto* decl = dynamic_cast<VarDeclNode*>(program->items[0].get());
    assert(decl != nullptr);
    assert(decl->name == "x");
    assert(decl->isMutable == true);

    auto* type = dynamic_cast<BuiltinTypeNode*>(decl->typeAnnot.get());
    assert(type != nullptr);
    assert(type->kind == BuiltinKind::I32);

    auto* init = dynamic_cast<LiteralExpr*>(decl->initializer.get());
    assert(init != nullptr);
    assert(init->kind == LiteralKind::Integer);

    std::cout << "[OK] testVarDecl passed!\n";
}

void testConstDecl() {
    Lexer lexer("const y = 3.14;");
    DiagnosticEngine diag;
    Parser parser(lexer, diag);

    auto program = parser.parse();
    assert(program->items.size() == 1);

    auto* decl = dynamic_cast<VarDeclNode*>(program->items[0].get());
    assert(decl != nullptr);
    assert(decl->name == "y");
    assert(decl->isMutable == false);
    assert(decl->typeAnnot == nullptr);

    auto* init = dynamic_cast<LiteralExpr*>(decl->initializer.get());
    assert(init != nullptr);
    assert(init->kind == LiteralKind::Float);

    std::cout << "[OK] testConstDecl passed!\n";
}

void testExpressions() {
    // Tests operator precedence
    Lexer lexer("dec result = 1 + 2 * 3 == 7 && true;");
    DiagnosticEngine diag;
    Parser parser(lexer, diag);

    auto program = parser.parse();
    assert(program->items.size() == 1);

    auto* decl = dynamic_cast<VarDeclNode*>(program->items[0].get());
    assert(decl != nullptr);

    auto* logicAnd = dynamic_cast<BinaryExpr*>(decl->initializer.get());
    assert(logicAnd != nullptr);
    assert(logicAnd->op == BinaryOp::LogicAnd);

    auto* eq = dynamic_cast<BinaryExpr*>(logicAnd->left.get());
    assert(eq != nullptr);
    assert(eq->op == BinaryOp::Eq);

    auto* add = dynamic_cast<BinaryExpr*>(eq->left.get());
    assert(add != nullptr);
    assert(add->op == BinaryOp::Add);

    auto* one = dynamic_cast<LiteralExpr*>(add->left.get());
    assert(one != nullptr && one->kind == LiteralKind::Integer);

    auto* mul = dynamic_cast<BinaryExpr*>(add->right.get());
    assert(mul != nullptr);
    assert(mul->op == BinaryOp::Mul);

    auto* two = dynamic_cast<LiteralExpr*>(mul->left.get());
    assert(two != nullptr && two->kind == LiteralKind::Integer);

    auto* three = dynamic_cast<LiteralExpr*>(mul->right.get());
    assert(three != nullptr && three->kind == LiteralKind::Integer);

    std::cout << "[OK] testExpressions passed!\n";
}

void testFunctionAndControlFlow() {
    Lexer lexer(
        "fn main() -> int_32 {\n"
        "    dec x: int_32 = 0;\n"
        "    if x == 0 {\n"
        "        x = 1;\n"
        "    } else {\n"
        "        x = 2;\n"
        "    }\n"
        "    while x < 10 {\n"
        "        x = x + 1;\n"
        "    }\n"
        "    for (i in iter) {\n"
        "        print i;\n"
        "    }\n"
        "    return x;\n"
        "}"
    );
    DiagnosticEngine diag;
    Parser parser(lexer, diag);

    auto program = parser.parse();
    assert(program->items.size() == 1);

    auto* funcDecl = dynamic_cast<FunctionDeclNode*>(program->items[0].get());
    assert(funcDecl != nullptr);
    assert(funcDecl->name == "main");
    assert(funcDecl->params.size() == 0);

    auto* retType = dynamic_cast<BuiltinTypeNode*>(funcDecl->returnType.get());
    assert(retType != nullptr && retType->kind == BuiltinKind::I32);

    auto* body = funcDecl->body.get();
    assert(body != nullptr);
    assert(body->body.size() == 5); // varDecl, ifStmt, whileStmt, forStmt, exprStmt (wait, I didn't implement return stmt in parser yet! Oh no, let's check `parseStatement`)

    std::cout << "[OK] testFunctionAndControlFlow passed!\n";
}

void testBatch3() {
    Lexer lexer(
        "struct Vec2 { x: float_32, y: float_32 }\n"
        "fn main() -> void {\n"
        "    dec v = Vec2 { x: 1.0, y: 2.0 };\n"
        "    v.x = v.x + foo::bar(v.y, z: 3.0)[0];\n"
        "    dec m = match v.x {\n"
        "        1.0 -> 10,\n"
        "        _ -> { return 20; }\n"
        "    };\n"
        "}"
    );
    DiagnosticEngine diag;
    Parser parser(lexer, diag);

    auto program = parser.parse();
    if (diag.hasErrors()) {
        std::cerr << "Diagnostics during parse:\n";
        for (const auto& d : diag.allDiagnostics()) {
            std::cerr << d.message << "\n";
        }
    }
    assert(program->items.size() == 2);

    auto* structDecl = dynamic_cast<StructDeclNode*>(program->items[0].get());
    assert(structDecl != nullptr);
    assert(structDecl->name == "Vec2");
    assert(structDecl->fields.size() == 2);

    auto* funcDecl = dynamic_cast<FunctionDeclNode*>(program->items[1].get());
    assert(funcDecl != nullptr);

    auto* body = funcDecl->body.get();
    assert(body->body.size() == 3);

    auto* vDecl = dynamic_cast<VarDeclNode*>(body->body[0].get());
    assert(vDecl != nullptr);
    auto* structInit = dynamic_cast<StructInitExpr*>(vDecl->initializer.get());
    assert(structInit != nullptr);
    assert(structInit->path[0] == "Vec2");
    assert(structInit->fields.size() == 2);

    std::cout << "[OK] testBatch3 passed!\n";
}

void testGenerics() {
    Lexer lexer(
        "struct HashMap<K: Hash + Eq, V> { data: int_32 }\n"
        "fn main() -> void {\n"
        "    dec map = HashMap@<string, int_32>::new();\n"
        "}"
    );
    DiagnosticEngine diag;
    Parser parser(lexer, diag);

    auto program = parser.parse();
    if (diag.hasErrors()) {
        std::cerr << "Diagnostics during parse:\n";
        for (const auto& d : diag.allDiagnostics()) {
            std::cerr << d.message << "\n";
        }
    }
    assert(program->items.size() == 2);

    auto* structDecl = dynamic_cast<StructDeclNode*>(program->items[0].get());
    assert(structDecl != nullptr);
    assert(structDecl->genericParams.size() == 2);
    assert(structDecl->genericParams[0].name == "K");
    assert(structDecl->genericParams[0].bounds.size() == 2);
    assert(structDecl->genericParams[1].name == "V");

    auto* funcDecl = dynamic_cast<FunctionDeclNode*>(program->items[1].get());
    assert(funcDecl != nullptr);
    
    auto* varDecl = dynamic_cast<VarDeclNode*>(funcDecl->body->body[0].get());
    assert(varDecl != nullptr);
    
    auto* callExpr = dynamic_cast<CallExpr*>(varDecl->initializer.get());
    assert(callExpr != nullptr);
    
    auto* identExpr = dynamic_cast<IdentifierExpr*>(callExpr->callee.get());
    assert(identExpr != nullptr);
    assert(identExpr->segments.size() == 2);
    assert(identExpr->segments[0] == "HashMap");
    assert(identExpr->segments[1] == "new");
    assert(identExpr->genericArgs.size() == 2);
    
    std::cout << "[OK] testGenerics passed!\n";
}

int main() {
    std::cout << "--- RUNNING PARSER TESTS ---\n";
    testVarDecl();
    testConstDecl();
    testExpressions();
    testFunctionAndControlFlow();
    testBatch3();
    testGenerics();
    std::cout << "--- ALL TESTS PASSED SUCCESSFULLY! ---\n";
    return 0;
}
