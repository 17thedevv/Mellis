#pragma once
#include "fdlang/FrontEnd/Lexer.h"
#include "fdlang/AST/ASTNode.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/StmtNode.h"
#include "fdlang/AST/TypeNode.h"
#include "fdlang/AST/ProgramNode.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/Support/Diagnostic.h"
#include <memory>
#include <string_view>

namespace fl {

class Parser {
private:
    Lexer& lexer;      // Tham chiếu đến cỗ máy Lexer (Pull-based)
    DiagnosticEngine& diag; // Engine báo lỗi
    Token current;     // Token đang đứng hiện tại
    Token peek;        // Token đi trước 1 bước (để nhìn trộm, giúp rẽ nhánh)

    // Exception dùng cục bộ để panic mode recovery
    struct ParseError {};
    
    // Đồng bộ hoá sau lỗi để tiếp tục phân tích
    void synchronize();

    // --- Các hàm thao tác con trỏ Token ---
    void advance();
    bool check(TokenType type) const;
    bool match(TokenType type);
    Token consume(TokenType type, const char* errorMessage);

    // Program & Items
    std::unique_ptr<ItemNode> parseDeclaration();
    std::unique_ptr<StmtNode> parseStatement();

    // Declarations
    std::unique_ptr<DeclNode> parseVarDecl();
    std::unique_ptr<DeclNode> parseFunctionDecl();
    std::unique_ptr<DeclNode> parseStructDecl();
    std::unique_ptr<DeclNode> parseEnumDecl();
    std::unique_ptr<DeclNode> parseTraitDecl();
    std::unique_ptr<DeclNode> parseImplDecl();

    // Generics
    std::vector<GenericParamNode> parseGenericParams();

    // Statements
    std::unique_ptr<StmtNode> parseAssignmentStatement();
    std::unique_ptr<StmtNode> parseExpressionStatement();
    std::unique_ptr<BlockStmtNode> parseBlockStatement();
    std::unique_ptr<StmtNode> parseIfStatement();
    std::unique_ptr<StmtNode> parseWhileStatement();
    std::unique_ptr<StmtNode> parseForStatement();
    std::unique_ptr<StmtNode> parseReturnStatement();
    std::unique_ptr<StmtNode> parseBreakStatement();
    std::unique_ptr<StmtNode> parseContinueStatement();
    std::unique_ptr<StmtNode> parsePrintStatement();

    // Type parsing
    std::unique_ptr<TypeNode> parseType();
    std::unique_ptr<TypeNode> parseNamedType();

    // Expressions (Fully granular precedence)
    std::unique_ptr<ExprNode> parseValuePath();
    std::unique_ptr<ExprNode> parseExpression(bool allowStructLiteral = true);
    std::unique_ptr<ExprNode> parseAssignment(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseRange(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseLogicalOr(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseLogicalAnd(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseBitwiseOr(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseBitwiseXor(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseBitwiseAnd(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseEquality(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseComparison(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseShift(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseTerm(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseFactor(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseCast(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parseUnary(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parsePostfix(bool allowStructLiteral);
    std::unique_ptr<ExprNode> parsePrimary(bool allowStructLiteral);

    // Advanced Expressions & Patterns
    std::unique_ptr<ExprNode> parseMatchExpr();
    std::unique_ptr<ExprNode> parseLambdaExpr();
    std::unique_ptr<PatternNode> parsePattern();
    
public:
    explicit Parser(Lexer& lexer, DiagnosticEngine& diag);
    
    // Hàm khởi chạy toàn bộ quá trình phân tích
    std::unique_ptr<ProgramNode> parse();
};

} // namespace fl