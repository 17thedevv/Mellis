#include <iostream>
#include <cassert>
#include "fdlang/Frontend/Lexer.h"
#include "fdlang/Frontend/Parser.h"
#include "fdlang/Frontend/SemanticAnalyzer.h"

using namespace fl;

void testValidVariableDeclarations() {
    std::string code = "dec a = 10; dec b = a;";
    
    Lexer lexer(code);
    DiagnosticEngine diag; Parser parser(lexer, diag);
    auto ast = parser.parse();
    
    SemanticAnalyzer semanticAnalyzer;
    ast->accept(&semanticAnalyzer); // Sẽ gọi exit() nếu có lỗi
    
    // Nếu code chạy được đến đây tức là Semantic Analyzer không ném lỗi
    const auto& symTable = semanticAnalyzer.getSymbolTable();
    
    std::cout << "[PASS] testValidVariableDeclarations\n";
}

int main() {
    std::cout << "--- CHAY TEST SEMANTIC ANALYZER ---\n";
    
    testValidVariableDeclarations();
    
    std::cout << "Tat ca cac test deu PASS!\n";
    return 0;
}
