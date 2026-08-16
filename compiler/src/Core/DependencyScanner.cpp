#include "mellis/Core/DependencyScanner.h"
#include "mellis/FrontEnd/Lexer.h"
#include <iostream>

namespace fl {

std::vector<std::string> DependencyScanner::scanDependencies(std::string_view source) {
    std::vector<std::string> dependencies;
    Lexer lexer(source);
    
    // We do not want the lexer to emit errors to diagnostic engine, so we just run it.
    // Lexer inherently skips whitespace and comments, so we are safe from them.
    
    Token token = lexer.nextToken();
    while (token.type != TokenType::END_OF_FILE) {
        // Early Exit Optimization:
        // Most declarations usually mean we are past the import section.
        // We stop scanning if we hit `fn`, `struct`, `impl`, `enum`, `trait`, etc.
        if (token.type == TokenType::KW_FN || 
            token.type == TokenType::KW_STRUCT || 
            token.type == TokenType::KW_ENUM || 
            token.type == TokenType::KW_IMPL || 
            token.type == TokenType::KW_TRAIT ||
            token.type == TokenType::KW_MACRO) {
            break;
        }

        if (token.type == TokenType::KW_USE) {
            std::string path;
            token = lexer.nextToken();
            while (token.type == TokenType::IDENTIFIER || token.type == TokenType::COLON_COLON) {
                if (token.type == TokenType::IDENTIFIER) {
                    path += std::string(token.text);
                } else if (token.type == TokenType::COLON_COLON) {
                    path += "::";
                }
                token = lexer.nextToken();
            }
            // `use a::b;` or `use a::b as c;`
            // If path ends with `::`, it might be a syntax error, but we just trim or ignore.
            if (!path.empty()) {
                dependencies.push_back(path);
            }
            continue; // We already advanced token
        }
        
        token = lexer.nextToken();
    }

    return dependencies;
}

} // namespace fl
