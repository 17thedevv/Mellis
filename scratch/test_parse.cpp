#include <iostream>
#include <string>
#include mellis/FrontEnd/Lexer.h
#include mellis/FrontEnd/Parser.h
#include mellis/FrontEnd/DiagnosticEngine.h
using namespace fl;
int main() {
    DiagnosticEngine diag;
    std::string src = export trait Eq {};
    Lexer lexer(src);
    Parser parser(lexer, diag);
    try {
        auto decl = parser.parseDeclaration();
        if (decl) std::cout << Parsed successfully!\\n;
        else std::cout << Returned null!\\n;
    } catch(const std::exception& e) {
        std::cout << Exception:  << e.what() << \\n;
    }
    return 0;
}

