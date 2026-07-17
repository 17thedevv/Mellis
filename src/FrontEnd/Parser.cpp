#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/FrontEnd/ASTVisitor.h"
#include "fdlang/AST/PatternNode.h"
#include <iostream>

namespace fl {

Parser::Parser(Lexer& lexer, DiagnosticEngine& diag) : lexer(lexer), diag(diag) {
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

bool Parser::match(TokenType type) {
    if (check(type)) {
        advance();
        return true;
    }
    return false;
}

Token Parser::consume(TokenType type, const char* errorMessage) {
    if (check(type)) {
        Token t = current;
        advance();
        return t;
    }
    SourceLocation loc{current.byteOffset, 0, 0};
    diag.error(loc, errorMessage);
    throw ParseError();
}

void Parser::synchronize() {
    advance();
    while (current.type != TokenType::END_OF_FILE) {
        if (current.type == TokenType::SEMI) return;
        switch (current.type) {
            case TokenType::KW_DEC: case TokenType::KW_CONST: case TokenType::KW_FN:
            case TokenType::KW_IF: case TokenType::KW_WHILE: case TokenType::KW_FOR:
            case TokenType::KW_RETURN: case TokenType::KW_MATCH: case TokenType::KW_STRUCT:
            case TokenType::KW_ENUM: case TokenType::KW_TRAIT: case TokenType::KW_IMPL:
                return;
            default: ;
        }
        advance();
    }
}

std::unique_ptr<ProgramNode> Parser::parse() {
    auto program = std::make_unique<ProgramNode>();
    while (current.type != TokenType::END_OF_FILE) {
        try {
            program->items.push_back(parseDeclaration());
        } catch (const ParseError&) {
            synchronize();
        }
    }
    return program;
}

// ==========================================
// GENERICS
// ==========================================

std::vector<GenericParamNode> Parser::parseGenericParams() {
    std::vector<GenericParamNode> params;
    if (match(TokenType::LESS_THAN)) {
        if (!check(TokenType::GREATER_THAN)) {
            do {
                GenericParamNode param;
                param.loc = SourceLocation{current.byteOffset, 0, 0};
                Token nameTok = consume(TokenType::IDENTIFIER, "Expected generic parameter name");
                param.name = nameTok.text;
                
                if (match(TokenType::COLON)) {
                    do {
                        param.bounds.push_back(parseNamedType());
                    } while (match(TokenType::PLUS));
                }
                params.push_back(std::move(param));
            } while (match(TokenType::COMMA));
        }
        consume(TokenType::GREATER_THAN, "Expected '>' after generic parameters");
    }
    return params;
}

// ==========================================
// DECLARATIONS
// ==========================================

std::unique_ptr<ItemNode> Parser::parseDeclaration() {
    switch (current.type) {
        case TokenType::KW_DEC:
        case TokenType::KW_CONST: return parseVarDecl();
        case TokenType::KW_FN: return parseFunctionDecl();
        case TokenType::KW_STRUCT: return parseStructDecl();
        case TokenType::KW_ENUM: return parseEnumDecl();
        case TokenType::KW_TRAIT: return parseTraitDecl();
        case TokenType::KW_IMPL: return parseImplDecl();
        default: return parseStatement();
    }
}

std::unique_ptr<DeclNode> Parser::parseVarDecl() {
    auto varDecl = std::make_unique<VarDeclNode>();
    varDecl->loc = SourceLocation{current.byteOffset, 0, 0};
    if (match(TokenType::KW_EXPORT)) varDecl->isExported = true;
    if (match(TokenType::KW_CONST)) varDecl->isMutable = false;
    else { consume(TokenType::KW_DEC, "Expected 'dec' or 'const'"); varDecl->isMutable = true; }

    Token nameTok = consume(TokenType::IDENTIFIER, "Expected variable name");
    varDecl->name = nameTok.text;
    if (match(TokenType::COLON)) varDecl->typeAnnot = parseType();
    
    consume(TokenType::EQUAL, "Expected '=' after variable declaration");
    varDecl->initializer = parseExpression();
    consume(TokenType::SEMI, "Expected ';' after variable declaration");
    return varDecl;
}

std::unique_ptr<DeclNode> Parser::parseFunctionDecl() {
    auto funcDecl = std::make_unique<FunctionDeclNode>();
    funcDecl->loc = SourceLocation{current.byteOffset, 0, 0};

    if (match(TokenType::KW_EXPORT)) funcDecl->isExported = true;
    if (match(TokenType::KW_COMPTIME)) funcDecl->isComptime = true;
    if (match(TokenType::KW_ASYNC)) funcDecl->isAsync = true;

    consume(TokenType::KW_FN, "Expected 'fn'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected function name");
    funcDecl->name = nameTok.text;

    funcDecl->genericParams = parseGenericParams();

    consume(TokenType::L_PAREN, "Expected '(' after function name");
    if (!check(TokenType::R_PAREN)) {
        do {
            auto param = std::make_unique<ParamDeclNode>();
            param->loc = SourceLocation{current.byteOffset, 0, 0};
            if (check(TokenType::KW_SELF_VAL)) {
                param->isSelf = true;
                param->name = current.text;
                advance();
                consume(TokenType::COLON, "Expected ':' after 'self'");
                param->type = parseType();
            } else {
                Token pName = consume(TokenType::IDENTIFIER, "Expected parameter name");
                param->name = pName.text;
                consume(TokenType::COLON, "Expected ':' after parameter name");
                if (match(TokenType::DOT_DOT_DOT)) param->isVariadic = true;
                param->type = parseType();
            }
            funcDecl->params.push_back(std::move(param));
        } while (match(TokenType::COMMA));
    }
    consume(TokenType::R_PAREN, "Expected ')' after parameters");

    if (match(TokenType::ARROW)) funcDecl->returnType = parseType();
    funcDecl->body = parseBlockStatement();
    return funcDecl;
}

std::unique_ptr<DeclNode> Parser::parseStructDecl() {
    auto node = std::make_unique<StructDeclNode>();
    node->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_STRUCT, "Expected 'struct'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected struct name");
    node->name = nameTok.text;
    
    node->genericParams = parseGenericParams();
    
    consume(TokenType::L_BRACE, "Expected '{' for struct body");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto field = std::make_unique<StructFieldNode>();
        field->loc = SourceLocation{current.byteOffset, 0, 0};
        Token fName = consume(TokenType::IDENTIFIER, "Expected field name");
        field->name = fName.text;
        consume(TokenType::COLON, "Expected ':' after field name");
        field->type = parseType();
        node->fields.push_back(std::move(field));
        if (!match(TokenType::COMMA)) break;
    }
    consume(TokenType::R_BRACE, "Expected '}' to end struct body");
    return node;
}

std::unique_ptr<DeclNode> Parser::parseEnumDecl() {
    auto node = std::make_unique<EnumDeclNode>();
    node->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_ENUM, "Expected 'enum'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected enum name");
    node->name = nameTok.text;
    
    node->genericParams = parseGenericParams();
    
    consume(TokenType::L_BRACE, "Expected '{' for enum body");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto var = std::make_unique<EnumVariantNode>();
        var->loc = SourceLocation{current.byteOffset, 0, 0};
        Token vName = consume(TokenType::IDENTIFIER, "Expected variant name");
        var->name = vName.text;
        
        if (match(TokenType::L_PAREN)) {
            if (!check(TokenType::R_PAREN)) {
                do {
                    auto param = std::make_unique<ParamDeclNode>();
                    param->loc = SourceLocation{current.byteOffset, 0, 0};
                    if (check(TokenType::IDENTIFIER) && peek.type == TokenType::COLON) {
                        Token pName = consume(TokenType::IDENTIFIER, "");
                        param->name = pName.text;
                        consume(TokenType::COLON, "");
                    }
                    param->type = parseType();
                    var->fields.push_back(std::move(param));
                } while (match(TokenType::COMMA));
            }
            consume(TokenType::R_PAREN, "Expected ')' after variant fields");
        }
        node->variants.push_back(std::move(var));
        if (!match(TokenType::COMMA)) break;
    }
    consume(TokenType::R_BRACE, "Expected '}'");
    return node;
}

std::unique_ptr<DeclNode> Parser::parseTraitDecl() {
    auto node = std::make_unique<TraitDeclNode>();
    node->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_TRAIT, "Expected 'trait'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected trait name");
    node->name = nameTok.text;
    
    node->genericParams = parseGenericParams();

    consume(TokenType::L_BRACE, "Expected '{'");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto decl = parseFunctionDecl();
        auto funcDecl = std::unique_ptr<FunctionDeclNode>(dynamic_cast<FunctionDeclNode*>(decl.release()));
        if (funcDecl) node->methods.push_back(std::move(funcDecl));
    }
    consume(TokenType::R_BRACE, "Expected '}'");
    return node;
}

std::unique_ptr<DeclNode> Parser::parseImplDecl() {
    auto node = std::make_unique<ImplDeclNode>();
    node->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_IMPL, "Expected 'impl'");
    
    node->genericParams = parseGenericParams();
    
    node->selfType = parseType();
    if (match(TokenType::KW_FOR)) {
        node->traitType = std::move(node->selfType);
        node->selfType = parseType();
    }
    
    consume(TokenType::L_BRACE, "Expected '{'");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto decl = parseFunctionDecl();
        auto funcDecl = std::unique_ptr<FunctionDeclNode>(dynamic_cast<FunctionDeclNode*>(decl.release()));
        if (funcDecl) node->methods.push_back(std::move(funcDecl));
    }
    consume(TokenType::R_BRACE, "Expected '}'");
    return node;
}

// ==========================================
// STATEMENTS
// ==========================================

std::unique_ptr<StmtNode> Parser::parseStatement() {
    if (check(TokenType::KW_IF)) return parseIfStatement();
    if (check(TokenType::KW_WHILE)) return parseWhileStatement();
    if (check(TokenType::KW_FOR)) return parseForStatement();
    if (check(TokenType::KW_RETURN)) return parseReturnStatement();
    if (check(TokenType::KW_BREAK)) return parseBreakStatement();
    if (check(TokenType::KW_CONTINUE)) return parseContinueStatement();
    if (check(TokenType::KW_PRINT)) return parsePrintStatement();
    if (check(TokenType::L_BRACE)) return parseBlockStatement();
    return parseExpressionStatement();
}

std::unique_ptr<StmtNode> Parser::parseAssignmentStatement() {
    diag.error(SourceLocation{current.byteOffset, 0, 0}, "parseAssignmentStatement not implemented");
    throw ParseError();
}

std::unique_ptr<StmtNode> Parser::parseExpressionStatement() {
    auto expr = parseExpression();
    consume(TokenType::SEMI, "Expected ';' after expression");
    auto stmt = std::make_unique<ExprStmtNode>();
    stmt->expr = std::move(expr);
    return stmt;
}

std::unique_ptr<BlockStmtNode> Parser::parseBlockStatement() {
    auto block = std::make_unique<BlockStmtNode>();
    block->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::L_BRACE, "Expected '{' to start block");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        block->body.push_back(parseDeclaration());
    }
    consume(TokenType::R_BRACE, "Expected '}' to end block");
    return block;
}

std::unique_ptr<StmtNode> Parser::parseIfStatement() {
    auto ifStmt = std::make_unique<IfStmtNode>();
    ifStmt->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_IF, "Expected 'if'");
    ifStmt->condition = parseExpression(false); // Do not allow struct literals in condition
    ifStmt->thenBranch = parseBlockStatement();
    if (match(TokenType::KW_ELSE)) {
        if (check(TokenType::KW_IF)) ifStmt->elseBranch = parseIfStatement();
        else ifStmt->elseBranch = parseBlockStatement();
    }
    return ifStmt;
}

std::unique_ptr<StmtNode> Parser::parseWhileStatement() {
    auto whileStmt = std::make_unique<WhileStmtNode>();
    whileStmt->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_WHILE, "Expected 'while'");
    whileStmt->condition = parseExpression(false); // No struct literals
    whileStmt->body = parseBlockStatement();
    return whileStmt;
}

std::unique_ptr<StmtNode> Parser::parseForStatement() {
    auto forStmt = std::make_unique<ForStmtNode>();
    forStmt->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_FOR, "Expected 'for'");
    consume(TokenType::L_PAREN, "Expected '(' after 'for'");
    Token ident = consume(TokenType::IDENTIFIER, "Expected loop variable");
    forStmt->bindingName = ident.text;
    consume(TokenType::KW_IN, "Expected 'in' after loop variable");
    forStmt->iterable = parseExpression(true);
    consume(TokenType::R_PAREN, "Expected ')' after for condition");
    forStmt->body = parseBlockStatement();
    forStmt->kind = ForKind::ForEach;
    return forStmt;
}

std::unique_ptr<StmtNode> Parser::parseReturnStatement() {
    auto retStmt = std::make_unique<ReturnStmtNode>();
    retStmt->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_RETURN, "Expected 'return'");
    if (!check(TokenType::SEMI)) retStmt->value = parseExpression();
    consume(TokenType::SEMI, "Expected ';' after return value");
    return retStmt;
}

std::unique_ptr<StmtNode> Parser::parseBreakStatement() {
    auto stmt = std::make_unique<BreakStmtNode>();
    stmt->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_BREAK, "Expected 'break'");
    consume(TokenType::SEMI, "Expected ';' after break");
    return stmt;
}

std::unique_ptr<StmtNode> Parser::parseContinueStatement() {
    auto stmt = std::make_unique<ContinueStmtNode>();
    stmt->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_CONTINUE, "Expected 'continue'");
    consume(TokenType::SEMI, "Expected ';' after continue");
    return stmt;
}

std::unique_ptr<StmtNode> Parser::parsePrintStatement() {
    consume(TokenType::KW_PRINT, "Expected 'print'");
    auto expr = parseExpression();
    consume(TokenType::SEMI, "Expected ';' after print");
    auto stmt = std::make_unique<ExprStmtNode>();
    stmt->expr = std::move(expr);
    return stmt;
}

// ==========================================
// TYPE PARSING
// ==========================================

std::unique_ptr<TypeNode> Parser::parseType() {
    if (match(TokenType::MULTIPLY)) {
        auto node = std::make_unique<PointerTypeNode>();
        node->loc = SourceLocation{current.byteOffset, 0, 0};
        node->isMutable = match(TokenType::KW_RW);
        node->inner = parseType();
        return node;
    }
    if (match(TokenType::BIT_AND)) {
        auto node = std::make_unique<ReferenceTypeNode>();
        node->loc = SourceLocation{current.byteOffset, 0, 0};
        node->isMutable = match(TokenType::KW_RW);
        node->inner = parseType();
        return node;
    }
    if (match(TokenType::L_BRACKET)) {
        auto node = std::make_unique<ArrayTypeNode>();
        node->loc = SourceLocation{current.byteOffset, 0, 0};
        node->elementType = parseType();
        if (match(TokenType::COMMA)) {
            node->size = parseExpression(true);
        }
        consume(TokenType::R_BRACKET, "Expected ']' after array type");
        return node;
    }
    if (match(TokenType::L_PAREN)) {
        auto node = std::make_unique<TupleTypeNode>();
        node->loc = SourceLocation{current.byteOffset, 0, 0};
        if (!check(TokenType::R_PAREN)) {
            do {
                node->elements.push_back(parseType());
            } while (match(TokenType::COMMA) && !check(TokenType::R_PAREN));
        }
        consume(TokenType::R_PAREN, "Expected ')' after tuple type");
        return node;
    }
    if (check(TokenType::BUILTIN_TYPE)) {
        Token tok = consume(TokenType::BUILTIN_TYPE, "Expected builtin type");
        auto type = std::make_unique<BuiltinTypeNode>();
        type->loc = SourceLocation{tok.byteOffset, 0, 0};
        type->kind = tok.builtinKind;
        return type;
    }
    if (check(TokenType::IDENTIFIER)) return parseNamedType();
    
    diag.error(SourceLocation{current.byteOffset, 0, 0}, "Expected a type");
    throw ParseError();
}

std::unique_ptr<TypeNode> Parser::parseNamedType() {
    auto node = std::make_unique<NamedTypeNode>();
    node->loc = SourceLocation{current.byteOffset, 0, 0};
    
    do {
        Token idTok = consume(TokenType::IDENTIFIER, "Expected identifier in type path");
        node->segments.push_back(idTok.text);
        
        if (match(TokenType::GENERIC_START)) {
            if (!check(TokenType::GREATER_THAN)) {
                do {
                    node->genericArgs.push_back(parseType());
                } while (match(TokenType::COMMA));
            }
            consume(TokenType::GREATER_THAN, "Expected '>' after generic arguments");
        }
    } while (match(TokenType::COLON_COLON));
    
    return node;
}

// ==========================================
// EXPRESSIONS (RECURSIVE DESCENT)
// ==========================================

std::unique_ptr<ExprNode> Parser::parseExpression(bool allowStructLiteral) {
    return parseAssignment(allowStructLiteral);
}

std::unique_ptr<ExprNode> Parser::parseAssignment(bool allowStructLiteral) {
    auto expr = parseRange(allowStructLiteral);
    if (check(TokenType::EQUAL) || check(TokenType::PLUS_ASSIGN) || check(TokenType::MINUS_ASSIGN) ||
        check(TokenType::STAR_ASSIGN) || check(TokenType::SLASH_ASSIGN) || check(TokenType::PERC_ASSIGN) ||
        check(TokenType::BIT_AND_ASSIGN) || check(TokenType::BIT_OR_ASSIGN) || check(TokenType::BIT_XOR_ASSIGN) ||
        check(TokenType::LSHIFT_ASSIGN) || check(TokenType::RSHIFT_ASSIGN)) {
        Token op = current;
        advance();
        auto value = parseAssignment(allowStructLiteral);
        auto assignExpr = std::make_unique<AssignExpr>();
        if (op.type == TokenType::EQUAL) assignExpr->op = AssignOp::Assign;
        else if (op.type == TokenType::PLUS_ASSIGN) assignExpr->op = AssignOp::AddAssign;
        else if (op.type == TokenType::MINUS_ASSIGN) assignExpr->op = AssignOp::SubAssign;
        else if (op.type == TokenType::STAR_ASSIGN) assignExpr->op = AssignOp::MulAssign;
        else if (op.type == TokenType::SLASH_ASSIGN) assignExpr->op = AssignOp::DivAssign;
        else if (op.type == TokenType::PERC_ASSIGN) assignExpr->op = AssignOp::ModAssign;
        else if (op.type == TokenType::BIT_AND_ASSIGN) assignExpr->op = AssignOp::BitAndAssign;
        else if (op.type == TokenType::BIT_OR_ASSIGN) assignExpr->op = AssignOp::BitOrAssign;
        else if (op.type == TokenType::BIT_XOR_ASSIGN) assignExpr->op = AssignOp::BitXorAssign;
        else if (op.type == TokenType::LSHIFT_ASSIGN) assignExpr->op = AssignOp::LShiftAssign;
        else if (op.type == TokenType::RSHIFT_ASSIGN) assignExpr->op = AssignOp::RShiftAssign;
        assignExpr->lvalue = std::move(expr);
        assignExpr->value = std::move(value);
        return assignExpr;
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseRange(bool allowStructLiteral) {
    auto expr = parseLogicalOr(allowStructLiteral);
    if (check(TokenType::DOT_DOT) || check(TokenType::DOT_DOT_EQ)) {
        Token op = current;
        advance();
        auto right = parseLogicalOr(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = op.type == TokenType::DOT_DOT ? BinaryOp::Range : BinaryOp::RangeInc;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        return bin;
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseLogicalOr(bool allowStructLiteral) {
    auto expr = parseLogicalAnd(allowStructLiteral);
    while (check(TokenType::LOGICAL_OR)) {
        advance();
        auto right = parseLogicalAnd(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = BinaryOp::LogicOr;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseLogicalAnd(bool allowStructLiteral) {
    auto expr = parseBitwiseOr(allowStructLiteral);
    while (check(TokenType::LOGICAL_AND)) {
        advance();
        auto right = parseBitwiseOr(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = BinaryOp::LogicAnd;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseBitwiseOr(bool allowStructLiteral) {
    auto expr = parseBitwiseXor(allowStructLiteral);
    while (check(TokenType::BIT_OR)) {
        advance();
        auto right = parseBitwiseXor(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = BinaryOp::BitOr;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseBitwiseXor(bool allowStructLiteral) {
    auto expr = parseBitwiseAnd(allowStructLiteral);
    while (check(TokenType::BIT_XOR)) {
        advance();
        auto right = parseBitwiseAnd(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = BinaryOp::BitXor;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseBitwiseAnd(bool allowStructLiteral) {
    auto expr = parseEquality(allowStructLiteral);
    while (check(TokenType::BIT_AND)) {
        advance();
        auto right = parseEquality(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = BinaryOp::BitAnd;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseEquality(bool allowStructLiteral) {
    auto expr = parseComparison(allowStructLiteral);
    while (check(TokenType::EQUAL_EQUAL) || check(TokenType::NOT_EQUAL)) {
        Token op = current;
        advance();
        auto right = parseComparison(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = (op.type == TokenType::EQUAL_EQUAL) ? BinaryOp::Eq : BinaryOp::Ne;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseComparison(bool allowStructLiteral) {
    auto expr = parseShift(allowStructLiteral);
    while (check(TokenType::LESS_THAN) || check(TokenType::LESS_THAN_EQUAL) ||
           check(TokenType::GREATER_THAN) || check(TokenType::GREATER_THAN_EQUAL)) {
        Token op = current;
        advance();
        auto right = parseShift(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        if (op.type == TokenType::LESS_THAN) bin->op = BinaryOp::Lt;
        else if (op.type == TokenType::LESS_THAN_EQUAL) bin->op = BinaryOp::Le;
        else if (op.type == TokenType::GREATER_THAN) bin->op = BinaryOp::Gt;
        else if (op.type == TokenType::GREATER_THAN_EQUAL) bin->op = BinaryOp::Ge;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseShift(bool allowStructLiteral) {
    auto expr = parseTerm(allowStructLiteral);
    while (check(TokenType::LSHIFT) || check(TokenType::RSHIFT)) {
        Token op = current;
        advance();
        auto right = parseTerm(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = (op.type == TokenType::LSHIFT) ? BinaryOp::LShift : BinaryOp::RShift;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseTerm(bool allowStructLiteral) {
    auto expr = parseFactor(allowStructLiteral);
    while (check(TokenType::PLUS) || check(TokenType::MINUS)) {
        Token op = current;
        advance();
        auto right = parseFactor(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        bin->op = (op.type == TokenType::PLUS) ? BinaryOp::Add : BinaryOp::Sub;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseFactor(bool allowStructLiteral) {
    auto expr = parseCast(allowStructLiteral);
    while (check(TokenType::MULTIPLY) || check(TokenType::DIVIDE) || check(TokenType::MODULO)) {
        Token op = current;
        advance();
        auto right = parseCast(allowStructLiteral);
        auto bin = std::make_unique<BinaryExpr>();
        if (op.type == TokenType::MULTIPLY) bin->op = BinaryOp::Mul;
        else if (op.type == TokenType::DIVIDE) bin->op = BinaryOp::Div;
        else if (op.type == TokenType::MODULO) bin->op = BinaryOp::Mod;
        bin->left = std::move(expr);
        bin->right = std::move(right);
        expr = std::move(bin);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseCast(bool allowStructLiteral) {
    auto expr = parseUnary(allowStructLiteral);
    while (match(TokenType::KW_AS)) {
        auto castExpr = std::make_unique<CastExpr>();
        castExpr->expr = std::move(expr);
        castExpr->targetType = parseType();
        expr = std::move(castExpr);
    }
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseUnary(bool allowStructLiteral) {
    if (check(TokenType::MINUS) || check(TokenType::BANG) || check(TokenType::BIT_NOT) ||
        check(TokenType::MULTIPLY) || check(TokenType::BIT_AND)) {
        Token op = current;
        advance();
        auto expr = std::make_unique<UnaryExpr>();
        expr->loc = SourceLocation{op.byteOffset, 0, 0};
        if (op.type == TokenType::MINUS) expr->op = UnaryOp::Neg;
        else if (op.type == TokenType::BANG) expr->op = UnaryOp::Not;
        else if (op.type == TokenType::BIT_NOT) expr->op = UnaryOp::BitNot;
        else if (op.type == TokenType::MULTIPLY) expr->op = UnaryOp::Deref;
        else if (op.type == TokenType::BIT_AND) {
            expr->op = match(TokenType::KW_RW) ? UnaryOp::RefMut : UnaryOp::Ref;
        }
        expr->operand = parseUnary(allowStructLiteral);
        return expr;
    }
    return parsePostfix(allowStructLiteral);
}

std::unique_ptr<ExprNode> Parser::parsePostfix(bool allowStructLiteral) {
    auto expr = parsePrimary(allowStructLiteral);
    
    while (true) {
        if (match(TokenType::L_PAREN)) {
            // Function Call
            auto callExpr = std::make_unique<CallExpr>();
            callExpr->loc = SourceLocation{current.byteOffset, 0, 0};
            callExpr->callee = std::move(expr);
            if (!check(TokenType::R_PAREN)) {
                do {
                    CallArgNode arg;
                    arg.loc = SourceLocation{current.byteOffset, 0, 0};
                    if (check(TokenType::IDENTIFIER) && peek.type == TokenType::COLON) {
                        Token idTok = consume(TokenType::IDENTIFIER, "");
                        consume(TokenType::COLON, "");
                        arg.label = idTok.text;
                    }
                    arg.value = parseExpression(true);
                    callExpr->args.push_back(std::move(arg));
                } while (match(TokenType::COMMA));
            }
            consume(TokenType::R_PAREN, "Expected ')' after function arguments");
            expr = std::move(callExpr);
        } else if (match(TokenType::L_BRACKET)) {
            // Array Indexing
            auto idxExpr = std::make_unique<IndexExpr>();
            idxExpr->loc = SourceLocation{current.byteOffset, 0, 0};
            idxExpr->base = std::move(expr);
            idxExpr->index = parseExpression(true);
            consume(TokenType::R_BRACKET, "Expected ']' after index");
            expr = std::move(idxExpr);
        } else if (match(TokenType::DOT)) {
            // Member Access or Method Call
            Token memberTok = consume(TokenType::IDENTIFIER, "Expected member name");
            
            // Check for Generic Arguments like `a.foo::<I32>()`? We don't have turbofish yet, but we might.
            // Let's assume standard `a.foo()` for now.
            if (match(TokenType::L_PAREN)) {
                auto methodCall = std::make_unique<MethodCallExpr>();
                methodCall->loc = SourceLocation{current.byteOffset, 0, 0};
                methodCall->object = std::move(expr);
                methodCall->methodName = memberTok.text;
                
                if (!check(TokenType::R_PAREN)) {
                    do {
                        CallArgNode arg;
                        arg.loc = SourceLocation{current.byteOffset, 0, 0};
                        if (check(TokenType::IDENTIFIER) && peek.type == TokenType::COLON) {
                            Token idTok = consume(TokenType::IDENTIFIER, "");
                            consume(TokenType::COLON, "");
                            arg.label = idTok.text;
                        }
                        arg.value = parseExpression(true);
                        methodCall->args.push_back(std::move(arg));
                    } while (match(TokenType::COMMA));
                }
                consume(TokenType::R_PAREN, "Expected ')' after method arguments");
                expr = std::move(methodCall);
            } else {
                auto memExpr = std::make_unique<MemberExpr>();
                memExpr->loc = SourceLocation{current.byteOffset, 0, 0};
                memExpr->object = std::move(expr);
                memExpr->member = memberTok.text;
                expr = std::move(memExpr);
            }
        } else if (allowStructLiteral && check(TokenType::L_BRACE)) {
            // Struct Initialization
            // This is only valid if `expr` is an IdentifierExpr representing a path.
            if (auto idExpr = dynamic_cast<IdentifierExpr*>(expr.get())) {
                auto structInit = std::make_unique<StructInitExpr>();
                structInit->loc = SourceLocation{current.byteOffset, 0, 0};
                structInit->path = idExpr->segments;
                
                consume(TokenType::L_BRACE, "");
                while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
                    FieldInitNode fieldInit;
                    fieldInit.loc = SourceLocation{current.byteOffset, 0, 0};
                    Token fName = consume(TokenType::IDENTIFIER, "Expected field name");
                    fieldInit.name = fName.text;
                    consume(TokenType::COLON, "Expected ':' after field name");
                    fieldInit.value = parseExpression(true);
                    structInit->fields.push_back(std::move(fieldInit));
                    if (!match(TokenType::COMMA)) break;
                }
                consume(TokenType::R_BRACE, "Expected '}' after struct literal fields");
                expr = std::move(structInit);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseValuePath() {
    auto ident = std::make_unique<IdentifierExpr>();
    ident->loc = SourceLocation{current.byteOffset, 0, 0};
    do {
        Token idTok = consume(TokenType::IDENTIFIER, "Expected identifier in path");
        ident->segments.push_back(idTok.text);
        
        if (match(TokenType::GENERIC_START)) {
            if (!check(TokenType::GREATER_THAN)) {
                do {
                    ident->genericArgs.push_back(parseType());
                } while (match(TokenType::COMMA));
            }
            consume(TokenType::GREATER_THAN, "Expected '>' after generic arguments");
        }
    } while (match(TokenType::COLON_COLON));
    
    return ident;
}

std::unique_ptr<ExprNode> Parser::parseMatchExpr() {
    auto matchExpr = std::make_unique<MatchExpr>();
    matchExpr->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::KW_MATCH, "Expected 'match'");
    matchExpr->subject = parseExpression(false); // No struct literals in match subject
    
    consume(TokenType::L_BRACE, "Expected '{' for match body");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        MatchArmNode arm;
        arm.loc = SourceLocation{current.byteOffset, 0, 0};
        arm.pattern = parsePattern();
        consume(TokenType::ARROW, "Expected '->' after pattern");
        
        if (check(TokenType::L_BRACE)) {
            arm.body = parseBlockStatement();
            match(TokenType::COMMA); // optional comma
        } else {
            auto armExpr = parseExpression(true);
            auto stmt = std::make_unique<ExprStmtNode>();
            stmt->expr = std::move(armExpr);
            arm.body = std::move(stmt);
            consume(TokenType::COMMA, "Expected ',' after match arm expression");
        }
        matchExpr->arms.push_back(std::move(arm));
    }
    consume(TokenType::R_BRACE, "Expected '}'");
    return matchExpr;
}

std::unique_ptr<PatternNode> Parser::parsePattern() {

    if (check(TokenType::IDENTIFIER)) {
        Token tok = current;
        advance();
        if (tok.text == "_") {
            return std::make_unique<WildcardPatternNode>();
        } else {
            auto pat = std::make_unique<IdentifierPatternNode>();
            pat->segments.push_back(tok.text);
            while (match(TokenType::COLON_COLON)) {
                Token seg = consume(TokenType::IDENTIFIER, "Expected identifier");
                pat->segments.push_back(seg.text);
            }
            if (pat->segments.size() > 1 || check(TokenType::L_PAREN)) {
                // It's an Enum pattern (Variant)
                auto enumPat = std::make_unique<EnumPatternNode>();
                enumPat->path = std::move(pat->segments);
                if (check(TokenType::L_PAREN)) {
                    consume(TokenType::L_PAREN, "");
                    if (!check(TokenType::R_PAREN)) {
                        do {
                            enumPat->fields.push_back(parsePattern());
                        } while (match(TokenType::COMMA));
                    }
                    consume(TokenType::R_PAREN, "Expected ')'");
                }
                return enumPat;
            }
            return pat;
        }
    }
    
    if (check(TokenType::INTEGER_LITERAL) || check(TokenType::FLOAT_LITERAL) || check(TokenType::STRING_LITERAL) || check(TokenType::KW_TRUE) || check(TokenType::KW_FALSE)) {
        auto litPat = std::make_unique<LiteralPatternNode>();
        litPat->lit = std::unique_ptr<LiteralExpr>(static_cast<LiteralExpr*>(parsePrimary(true).release()));
        return litPat;
    }
    
    diag.error(SourceLocation{current.byteOffset, 0, 0}, "Expected a pattern (e.g., literal, identifier, or '_') in match arm");
    throw ParseError();
}

std::unique_ptr<ExprNode> Parser::parseLambdaExpr() {
    auto lambda = std::make_unique<LambdaExpr>();
    lambda->loc = SourceLocation{current.byteOffset, 0, 0};
    consume(TokenType::BIT_OR, "Expected '|'"); // Note: we use BIT_OR for lambda. Lexer might tokenize it as `|`.
    
    if (!check(TokenType::BIT_OR)) {
        do {
            auto param = std::make_unique<ParamDeclNode>();
            Token nameTok = consume(TokenType::IDENTIFIER, "Expected lambda param name");
            param->name = nameTok.text;
            if (match(TokenType::COLON)) {
                param->type = parseType();
            }
            lambda->params.push_back(std::move(param));
        } while (match(TokenType::COMMA));
    }
    consume(TokenType::BIT_OR, "Expected '|'");
    
    if (match(TokenType::ARROW)) {
        lambda->returnType = parseType();
    }
    
    if (check(TokenType::L_BRACE)) {
        lambda->body = parseBlockStatement();
    } else {
        auto expr = parseExpression(true);
        auto stmt = std::make_unique<ExprStmtNode>();
        stmt->expr = std::move(expr);
        lambda->body = std::move(stmt);
    }
    return lambda;
}

std::unique_ptr<ExprNode> Parser::parsePrimary(bool allowStructLiteral) {
    if (match(TokenType::INTEGER_LITERAL)) {
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Integer;
        return lit;
    }
    if (match(TokenType::FLOAT_LITERAL)) {
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Float;
        return lit;
    }
    if (match(TokenType::STRING_LITERAL)) {
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Str;
        return lit;
    }
    if (match(TokenType::KW_TRUE) || match(TokenType::KW_FALSE)) {
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Bool;
        return lit;
    }
    if (check(TokenType::IDENTIFIER)) {
        return parseValuePath();
    }
    if (check(TokenType::KW_MATCH)) {
        return parseMatchExpr();
    }
    if (check(TokenType::BIT_OR)) {
        return parseLambdaExpr();
    }
    if (match(TokenType::L_PAREN)) {
        auto expr = parseExpression(true);
        consume(TokenType::R_PAREN, "Expected ')' after expression");
        return expr;
    }

    diag.error(SourceLocation{current.byteOffset, 0, 0}, "Expected expression (e.g., literal, identifier, or '(')");
    throw ParseError();
}

} // namespace fl