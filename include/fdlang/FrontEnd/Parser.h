#pragma once
#include "fdlang/Frontend/Lexer.h"
#include "fdlang/AST/ASTNode.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/StmtNode.h"
#include <memory>
#include <string_view>

namespace fl {

class Parser {
private:
    Lexer& lexer;      // Tham chiếu đến cỗ máy Lexer (Pull-based)
    Token current;     // Token đang đứng hiện tại
    Token peek;        // Token đi trước 1 bước (để nhìn trộm, giúp rẽ nhánh)

    // --- Các hàm thao tác con trỏ Token ---
    void advance();
    bool check(TokenType type) const;
    bool match(TokenType type);
    Token consume(TokenType type, const char* errorMessage);

    // --- Các hàm xây dựng Cây (Parsing methods) ---
    
    // 1. Phân tích cấp cao nhất
    std::unique_ptr<StmtNode> parseStatement();
    
    // 2. Phân tích câu lệnh
    std::unique_ptr<VarDeclStmt> parseVarDeclaration();
    
    // 3. Phân tích biểu thức
    std::unique_ptr<ExprNode> parseExpression();
    std::unique_ptr<ExprNode> parsePrimary(); // Điểm chốt cuối: Số (Number) hoặc Biến (Identifier)

public:
    explicit Parser(Lexer& lexer);
    
    // Hàm khởi chạy toàn bộ quá trình phân tích
    std::unique_ptr<ProgramNode> parse();
};

} // namespace fl