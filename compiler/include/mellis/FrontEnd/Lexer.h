#pragma once
#include <string_view>
#include "Token.h"

namespace fl {

class Lexer {
private:
    std::string_view source; 
    uint32_t position;       

    // ── Line/column tracking ───────────────────────────────────────────────────
    uint32_t line_      = 1; // vị trí hiện tại (sau khi advance)
    uint32_t col_       = 1;
    uint32_t startLine_ = 1; // vị trí bắt đầu của token đang lex
    uint32_t startCol_  = 1;
    char peek() const;
    char peekNext() const;
    char peekNextNext() const;
    char advance();
    bool isAtEnd() const;
    bool match(char expected);

    // --- Bỏ qua khoảng trắng và comment ---
    void skipWhitespaceAndComments();

    // --- Khởi tạo Token ---
    Token makeToken(TokenType type, uint32_t startOffset, uint32_t length);
    Token errorToken(const char* message, uint32_t startOffset);

    // --- Rẽ nhánh xử lý ---
    Token identifierOrKeyword();
    Token numberLiteral();
    Token charLiteral(bool isByte);
    Token stringLiteral(bool isRaw, bool isByte);

public:
    explicit Lexer(std::string_view sourceCode);
    
    struct LexerState {
        uint32_t position;
        uint32_t line;
        uint32_t col;
        uint32_t startLine;
        uint32_t startCol;
    };
    LexerState saveState() const { return {position, line_, col_, startLine_, startCol_}; }
    void restoreState(const LexerState& state) {
        position = state.position;
        line_ = state.line;
        col_ = state.col;
        startLine_ = state.startLine;
        startCol_ = state.startCol;
    }

    // Hàm cốt lõi: Parser sẽ gọi hàm này liên tục để lấy Token
    Token nextToken(); 
};

} // namespace fl