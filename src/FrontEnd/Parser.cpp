#include "fdlang/Frontend/Parser.h"
#include <iostream>
#include <cstdlib>

namespace fl {

Parser::Parser(Lexer& lexer) : lexer(lexer) {
    // Khởi động mồi 2 nhịp để nạp đầy 'current' và 'peek'
    advance();
    advance();
}

void Parser::advance() {
    current = peek;
    peek = lexer.nextToken();
}

bool Parser::check(TokenType type) const {
    if (current.type == TokenType::END_OF_FILE) return false;
    return current.type == type;
}

// Nếu đúng loại Token cần tìm, tiến lên 1 bước và báo thành công
bool Parser::match(TokenType type) {
    if (check(type)) {
        advance();
        return true;
    }
    return false;
}

// Hàm cực kỳ quan trọng: "Bắt buộc phải có Token này, nếu không thì báo lỗi ngừng biên dịch"
Token Parser::consume(TokenType type, const char* errorMessage) {
    if (check(type)) {
        Token t = current;
        advance();
        return t;
    }
    
    // Nếu cú pháp sai (ví dụ thiếu dấu ;), báo lỗi và dừng ngay lập tức
    std::cerr << "Loi cu phap tai offset " << current.byteOffset << ": " << errorMessage << "\n";
    std::exit(65); // EX_DATAERR
}

// ==========================================
// CÁC HÀM XÂY DỰNG CÂY AST
// ==========================================

std::unique_ptr<ProgramNode> Parser::parse() {
    auto program = std::make_unique<ProgramNode>();
    
    // Lặp cho đến khi hết file
    while (current.type != TokenType::END_OF_FILE) {
        program->statements.push_back(parseStatement());
    }
    
    return program;
}

std::unique_ptr<StmtNode> Parser::parseStatement() {
    // Nếu thấy chữ 'dec', nhảy vào luồng phân tích khai báo biến
    if (match(TokenType::KW_DEC)) {
        return parseVarDeclaration();
    }

    // Nếu không phải từ khóa quen thuộc, báo lỗi
    std::cerr << "Loi: Cau lenh khong hop le tai offset " << current.byteOffset << "\n";
    std::exit(65);
}

std::unique_ptr<VarDeclStmt> Parser::parseVarDeclaration() {
    // 1. Phải có tên biến
    Token nameToken = consume(TokenType::IDENTIFIER, "Thieu ten bien sau 'dec'.");
    
    // 2. Phải có dấu '='
    consume(TokenType::EQUAL, "Thieu dau '=' trong khai bao bien.");
    
    // 3. Phân tích giá trị bên phải dấu '='
    std::unique_ptr<ExprNode> initializer = parseExpression();
    
    // 4. Bắt buộc kết thúc bằng dấu ';'
    consume(TokenType::SEMI, "Thieu dau ';' o cuoi cau lenh.");

    return std::make_unique<VarDeclStmt>(nameToken.text, std::move(initializer));
}

std::unique_ptr<ExprNode> Parser::parseExpression() {
    // Tạm thời biểu thức chỉ hỗ trợ đơn giản là 1 con số hoặc 1 biến
    return parsePrimary();
}

    std::unique_ptr<ExprNode> Parser::parsePrimary() {
    // Chỉ kiểm tra (check) chứ không dùng match để tránh bị nhảy cóc Token
    if (check(TokenType::NUMBER)) {
        Token num = current;
        advance(); // Lấy xong mới tiến lên
        return std::make_unique<NumberExpr>(num.text);
    }
    
    if (check(TokenType::IDENTIFIER)) {
        Token id = current;
        advance();
        return std::make_unique<IdentifierExpr>(id.text);
    }

    std::cerr << "Loi: Bieu thuc khong hop le tai offset " << current.byteOffset << "\n";
    std::exit(65);
}

} // namespace fl