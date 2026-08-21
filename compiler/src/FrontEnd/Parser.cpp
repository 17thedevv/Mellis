#include "mellis/FrontEnd/Parser.h"
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/AST/MacroNode.h"
#include "mellis/AST/MacroNode.h"
#include "mellis/Core/SourceManager.h"
#include <charconv>
#include <iostream>

namespace fl {

Parser::Parser(Lexer& lexer, DiagnosticEngine& diag, SourceManager* srcMgr, FileID fid) 
    : lexer(lexer), diag(diag), sourceMgr(srcMgr), fileId(fid) {
    advance();
    advance();
}

void Parser::advance() {
    prev = current;
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
    SourceLocation loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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

static bool isGenericEnd(TokenType type) {
    return type == TokenType::GREATER_THAN || type == TokenType::RSHIFT || type == TokenType::RSHIFT_ASSIGN || type == TokenType::GREATER_THAN_EQUAL;
}

void Parser::consumeGenericEnd() {
    if (match(TokenType::GREATER_THAN)) {
        return;
    }
    if (current.type == TokenType::RSHIFT) {
        current.type = TokenType::GREATER_THAN;
        current.text = std::string_view(current.text.data() + 1, current.text.length() - 1);
        current.byteOffset += 1;
        return;
    }
    if (current.type == TokenType::RSHIFT_ASSIGN) {
        current.type = TokenType::GREATER_THAN_EQUAL;
        current.text = std::string_view(current.text.data() + 1, current.text.length() - 1);
        current.byteOffset += 1;
        return;
    }
    if (current.type == TokenType::GREATER_THAN_EQUAL) {
        current.type = TokenType::EQUAL;
        current.text = std::string_view(current.text.data() + 1, current.text.length() - 1);
        current.byteOffset += 1;
        return;
    }
    diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected '>'");
    throw ParseError();
}

std::vector<GenericParamNode> Parser::parseGenericParams() {
    std::vector<GenericParamNode> params;
    if (match(TokenType::GENERIC_START) || match(TokenType::LESS_THAN)) {
        if (!isGenericEnd(current.type)) {
            do {
                GenericParamNode param;
                param.loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                if (current.type == TokenType::LIFETIME) {
                    param.kind = GenericParamKind::Lifetime;
                    param.name = current.text;
                    advance();
                    if (match(TokenType::COLON)) {
                        do {
                            if (current.type == TokenType::LIFETIME) {
                                auto ltNode = std::make_unique<LifetimeNode>();
                                ltNode->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                                ltNode->name = current.text;
                                advance();
                                param.bounds.push_back(std::move(ltNode));
                            } else {
                                diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected lifetime bound for a lifetime parameter");
                                throw ParseError();
                            }
                        } while (match(TokenType::PLUS));
                    }
                } else {
                    param.kind = GenericParamKind::Type;
                    Token nameTok = consume(TokenType::IDENTIFIER, "Expected generic parameter name or lifetime");
                    param.name = nameTok.text;
                    
                    if (match(TokenType::COLON)) {
                        do {
                            param.bounds.push_back(parseNamedType());
                        } while (match(TokenType::PLUS));
                    }
                }
                params.push_back(std::move(param));
            } while (match(TokenType::COMMA));
        }
        consumeGenericEnd();
    }
    return params;
}

std::vector<std::unique_ptr<TypeNode>> Parser::parseGenericArgs(std::vector<NamedTypeNode::AssociatedBinding>* bindings) {
    std::vector<std::unique_ptr<TypeNode>> args;
    if (match(TokenType::GENERIC_START) || match(TokenType::LESS_THAN)) {
        if (!isGenericEnd(current.type)) {
            do {
                if (bindings && current.type == TokenType::IDENTIFIER && peek.type == TokenType::EQUAL) {
                    Token nameTok = consume(TokenType::IDENTIFIER, "");
                    consume(TokenType::EQUAL, "");
                    NamedTypeNode::AssociatedBinding b;
                    b.name = nameTok.text;
                    b.type = parseType();
                    bindings->push_back(std::move(b));
                } else {
                    args.push_back(parseType());
                }
            } while (match(TokenType::COMMA));
        }
        consumeGenericEnd();
    }
    return args;
}

// ==========================================
// DECLARATIONS
// ==========================================


std::vector<AnnotationNode> Parser::parseAnnotations() {
    std::vector<AnnotationNode> annotations;
    while (match(TokenType::AT_BRACKET)) {
        do {
            AnnotationNode annot;
            // No loc on AnnotationNode
            Token nameTok = consume(TokenType::IDENTIFIER, "Expected annotation name");
            annot.name = nameTok.text;
            if (match(TokenType::L_PAREN)) {
                do {
                    AnnotationArg arg;
                    if (current.type == TokenType::IDENTIFIER && peek.type == TokenType::EQUAL) {
                        Token keyTok = consume(TokenType::IDENTIFIER, "Expected annotation arg key");
                        consume(TokenType::EQUAL, "Expected '=' after annotation arg key");
                        arg.key = std::string(keyTok.text);
                    }
                    arg.value = parseExpression(false);
                    annot.args.push_back(std::move(arg));
                } while (match(TokenType::COMMA));
                consume(TokenType::R_PAREN, "Expected ')' after annotation args");
            }
            annotations.push_back(std::move(annot));
        } while (match(TokenType::COMMA));
        consume(TokenType::R_BRACKET, "Expected ']' after annotations");
    }
    return annotations;
}

UseTreeNode Parser::parseUseTree() {
    UseTreeNode node;
    node.loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    if (match(TokenType::MULTIPLY)) {
        node.isGlob = true;
        return node;
    }
    if (match(TokenType::L_BRACE)) {
        do {
            node.children.push_back(parseUseTree());
        } while (match(TokenType::COMMA));
        consume(TokenType::R_BRACE, "Expected '}' in use tree");
        return node;
    }
    while (true) {
        if (current.type == TokenType::IDENTIFIER || current.type == TokenType::KW_PRINT || current.type == TokenType::STRING_LITERAL) {
            node.segments.push_back(current.text);
            advance();
        } else {
            diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected identifier or string in use path");
            throw ParseError();
        }
        
        if (check(TokenType::COLON_COLON)) {
            advance(); // consume ::
            if (current.type == TokenType::MULTIPLY || current.type == TokenType::L_BRACE) {
                break;
            }
        } else {
            break;
        }
    }
    
    std::cout << "[DEBUG] parseUseTree after segment: " << (int)current.type << " " << std::string(current.text) << "\n";
    if (current.type == TokenType::MULTIPLY || current.type == TokenType::L_BRACE || current.type == TokenType::IDENTIFIER || current.type == TokenType::STRING_LITERAL) {
        if (match(TokenType::MULTIPLY)) {
            node.isGlob = true;
            return node;
        }
        if (match(TokenType::L_BRACE)) {
            do {
                node.children.push_back(parseUseTree());
            } while (match(TokenType::COMMA));
            consume(TokenType::R_BRACE, "Expected '}' in use tree");
            return node;
        }
    }
    
    if (match(TokenType::KW_AS)) {
        Token aliasTok = consume(TokenType::IDENTIFIER, "Expected alias after 'as'");
        node.alias = aliasTok.text;
    }
    return node;
}

std::unique_ptr<DeclNode> Parser::parseTypeAliasDecl() {
    auto node = std::make_unique<TypeAliasDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_TYPE, "Expected 'type'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected type alias name");
    node->name = nameTok.text;
    node->genericParams = parseGenericParams();
    consume(TokenType::EQUAL, "Expected '='");
    node->aliasedType = parseType();
    consume(TokenType::SEMI, "Expected ';' after type alias");
    return node;
}

std::unique_ptr<DeclNode> Parser::parseModDecl() {
    auto node = std::make_unique<ModDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_MOD, "Expected 'mod'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected module name");
    node->name = nameTok.text;
    
    if (match(TokenType::SEMI)) {
        // Out-of-line module
        node->isOutlined = true;
        std::cout << "[DEBUG] parseModDecl: sourceMgr=" << sourceMgr << "\n";
        if (sourceMgr) {
            std::string modPath = sourceMgr->resolveModulePath(fileId, node->name);
            if (modPath.empty()) {
                diag.error(node->loc, "Could not find module file for '" + std::string(node->name) + "'");
            } else {
                FileID modFileId = sourceMgr->loadFile(modPath);
                if (modFileId != SourceManager::kInvalidFileID) {
                    std::string_view modSource = sourceMgr->getSource(modFileId);
                    Lexer modLexer(modSource);
                    Parser modParser(modLexer, diag, sourceMgr, modFileId);
                    auto modProg = modParser.parse();
                    for (auto& d : modProg->items) {
                        if (auto decl = dynamic_cast<DeclNode*>(d.get())) {
                            d.release();
                            node->decls.push_back(std::unique_ptr<DeclNode>(decl));
                        }
                    }
                }
            }
        } else {
            extern bool g_quiet; if (!g_quiet) std::cerr << "[DEBUG] HIT ELSE BRANCH! sourceMgr is " << sourceMgr << ", mod name is '" << node->name << "', at byteOffset=" << current.byteOffset << std::endl;
            diag.error(node->loc, "Out-of-line modules not supported without SourceManager");
        }
    } else {
        consume(TokenType::L_BRACE, "Expected '{' or ';' after module name");
        while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
            auto item = parseDeclaration();
            if (auto decl = dynamic_cast<DeclNode*>(item.get())) {
                item.release();
                node->decls.push_back(std::unique_ptr<DeclNode>(decl));
            } else {
                diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected a declaration inside module");
            }
        }
        consume(TokenType::R_BRACE, "Expected '}' after module body");
    }
    return node;
}

std::unique_ptr<DeclNode> Parser::parseUseDecl() {

    auto node = std::make_unique<UseDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_USE, "Expected 'use'");
    node->tree = parseUseTree();
    if (current.type != TokenType::SEMI) { std::cout << "[DEBUG] parseUseDecl expected SEMI but got: " << (int)current.type << " text: " << std::string(current.text) << "\n"; } consume(TokenType::SEMI, "Expected ';' after use declaration");
    return node;
}





std::unique_ptr<DeclNode> Parser::parseExternDecl() {
    auto node = std::make_unique<ExternDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_EXTERN, "Expected 'extern'");
    auto func = parseFunctionDecl(true); // true means allow empty body
    auto fd = dynamic_cast<FunctionDeclNode*>(func.get());
    if (!fd) {
        diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected function declaration after extern");
        throw ParseError();
    }
    fd->isUnsafe = true; // All extern functions are inherently unsafe
    func.release();
    node->func = std::unique_ptr<FunctionDeclNode>(fd);
    return node;
}



std::unique_ptr<ItemNode> Parser::parseDeclaration() {
    auto annots = parseAnnotations();
    Visibility visibility = Visibility::Internal;
    
    std::unique_ptr<DeclNode> decl;
    
    // Check export and extern modifiers first
    bool isExtern = false;
    if (match(TokenType::KW_EXPORT)) {
        visibility = Visibility::Public;
    }
    if (match(TokenType::KW_EXTERN)) {
        isExtern = true;
    }

    bool isFunction = false;
    if (current.type == TokenType::KW_FN || current.type == TokenType::KW_ASYNC || current.type == TokenType::KW_INTRINSIC) {
        isFunction = true;
    } else if (current.type == TokenType::KW_COMPTIME) {
        std::cout << "[DEBUG] parseDeclaration KW_COMPTIME. peek.type=" << (int)peek.type << "\n";
        if (peek.type == TokenType::KW_FN || peek.type == TokenType::KW_ASYNC || peek.type == TokenType::KW_INTRINSIC) {
            isFunction = true;
        }
    } else if (current.type == TokenType::KW_UNSAFE) {
        if (peek.type == TokenType::KW_FN || peek.type == TokenType::KW_INTRINSIC) {
            isFunction = true;
        }
    }
    
    if (current.type == TokenType::KW_DEC || current.type == TokenType::KW_CONST) {
        decl = parseVarDecl();
    } else if (isFunction) {
        decl = parseFunctionDecl(true); // Allow empty body for generic trait methods loaded from mlib
    } else if (current.type == TokenType::KW_STRUCT) {
        decl = parseStructDecl();
    } else if (current.type == TokenType::KW_ENUM) {
        decl = parseEnumDecl();
    } else if (current.type == TokenType::KW_TRAIT) {
        decl = parseTraitDecl();
    } else if (current.type == TokenType::KW_IMPL) {
        decl = parseImplDecl();
    } else if (current.type == TokenType::KW_USE) {
        decl = parseUseDecl();
    } else if (current.type == TokenType::KW_TYPE) {
        decl = parseTypeAliasDecl();
    } else if (current.type == TokenType::KW_MOD) {
        decl = parseModDecl();
    } else if (current.type == TokenType::KW_MACRO) {
        decl = parseMacroDecl();
    } else {
        if (!annots.empty() || visibility == Visibility::Public || isExtern) {
            diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Annotations or modifiers not attached to a declaration");
            throw ParseError();
        }
        return parseStatement();
    }
    
    if (decl) {
        decl->visibility = visibility;
        decl->annotations = std::move(annots);
        
        if (isExtern) {
            auto extNode = std::make_unique<ExternDeclNode>();
            extNode->loc = decl->loc;
            extNode->visibility = decl->visibility;
            if (auto funcDecl = dynamic_cast<FunctionDeclNode*>(decl.get())) {
                decl.release();
                extNode->func.reset(funcDecl);
                decl = std::move(extNode);
            } else {
                diag.error(decl->loc, "'extern' is only supported for functions");
            }
        }
    }
    return decl;
}

std::unique_ptr<DeclNode> Parser::parseVarDecl() {
    auto varDecl = std::make_unique<VarDeclNode>();
    varDecl->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    if (match(TokenType::KW_EXPORT)) varDecl->visibility = Visibility::Public;
    if (match(TokenType::KW_CONST)) varDecl->isMutable = false;
    else { consume(TokenType::KW_DEC, "Expected 'dec' or 'const'"); varDecl->isMutable = true; }

    varDecl->pattern = parsePattern();
    
    // Fallback variable name for backward compatibility with simple binding patterns
    if (auto bindPat = dynamic_cast<fl::IdentifierPatternNode*>(varDecl->pattern.get())) {
        varDecl->name = bindPat->segments.back(); // Assuming single segment for simple var decls
    } else {
        varDecl->name = "<pattern>";
    }

    if (match(TokenType::COLON)) varDecl->typeAnnot = parseType();
    
    if (match(TokenType::EQUAL)) {
        varDecl->initializer = parseExpression();
    }
    
    consume(TokenType::SEMI, "Expected ';' after variable declaration");
    return varDecl;
}

std::unique_ptr<DeclNode> Parser::parseFunctionDecl(bool allowEmptyBody) {
    auto funcDecl = std::make_unique<FunctionDeclNode>();
    funcDecl->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);

    if (match(TokenType::KW_EXPORT)) funcDecl->visibility = Visibility::Public;
    if (match(TokenType::KW_COMPTIME)) funcDecl->isComptime = true;
    if (match(TokenType::KW_ASYNC)) funcDecl->isAsync = true;
    if (match(TokenType::KW_UNSAFE)) funcDecl->isUnsafe = true;
    if (match(TokenType::KW_INTRINSIC)) funcDecl->isIntrinsic = true;
    
    consume(TokenType::KW_FN, "Expected 'fn'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected function name");
    funcDecl->name = nameTok.text;

    funcDecl->genericParams = parseGenericParams();

    consume(TokenType::L_PAREN, "Expected '(' after function name");
    if (!check(TokenType::R_PAREN)) {
        do {
            if (match(TokenType::DOT_DOT_DOT)) {
                funcDecl->isVariadic = true;
                break;
            }
            auto param = std::make_unique<ParamDeclNode>();
            param->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            if (match(TokenType::DOT_DOT_DOT)) {
                funcDecl->isVariadic = true;
                break; // '...' must be the last parameter
            } else if (check(TokenType::KW_SELF_VAL) || 
                       (check(TokenType::BIT_AND) && (peek.type == TokenType::KW_SELF_VAL || 
                                                       (peek.type == TokenType::KW_MUT) ||
                                                       (peek.type == TokenType::KW_RW)))) {
                param->isSelf = true;
                param->name = "self";
                
                bool isRef = false;
                bool isMut = false;
                if (match(TokenType::BIT_AND)) {
                    isRef = true;
                    if (match(TokenType::KW_MUT) || match(TokenType::KW_RW)) {
                        isMut = true;
                    }
                    consume(TokenType::KW_SELF_VAL, "Expected 'self' after '&' or '&mut'");
                } else {
                    param->name = current.text;
                    advance();
                }

                if (match(TokenType::COLON)) {
                    param->type = parseType();
                } else {
                    auto selfType = std::make_unique<NamedTypeNode>();
                    selfType->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                    selfType->segments.push_back("Self");
                    
                    if (isRef) {
                        auto refType = std::make_unique<ReferenceTypeNode>();
                        refType->loc = selfType->loc;
                        refType->inner = std::move(selfType);
                        refType->isMutable = isMut;
                        param->type = std::move(refType);
                    } else {
                        param->type = std::move(selfType);
                    }
                }
            } else {
                extern bool g_quiet; if (!g_quiet) std::cerr << "[DEBUG] current type=" << (int)current.type << " peek type=" << (int)peek.type << "\n";
                Token pName = consume(TokenType::IDENTIFIER, "Expected parameter name");
                param->name = pName.text;
                consume(TokenType::COLON, "Expected ':' after parameter name");
                param->type = parseType();
            }
            funcDecl->params.push_back(std::move(param));
        } while (match(TokenType::COMMA));
    }
    consume(TokenType::R_PAREN, "Expected ')' after parameters");

    if (match(TokenType::ARROW)) funcDecl->returnType = parseType();
    if (allowEmptyBody && match(TokenType::SEMI)) {
        // Empty body allowed for extern functions
    } else {
        funcDecl->body = parseBlockStatement();
    }
    funcDecl->endLoc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset + prev.text.length());
    return funcDecl;
}

std::unique_ptr<DeclNode> Parser::parseMacroDecl() {
    auto node = std::make_unique<MacroDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_MACRO, "Expected 'macro'");
    node->name = consume(TokenType::IDENTIFIER, "Expected macro name").text;

    consume(TokenType::L_PAREN, "Expected '(' after macro name");
    if (!check(TokenType::R_PAREN)) {
        do {
            MacroParamNode param;
            param.loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            consume(TokenType::AT, "Expected '@' before macro parameter name");
            param.name = consume(TokenType::IDENTIFIER, "Expected macro parameter name").text;
            consume(TokenType::COLON, "Expected ':' after macro parameter name");
            Token fragTok = consume(TokenType::IDENTIFIER, "Expected fragment specifier (e.g., 'expr', 'stmt', 'ident')");
            if (fragTok.text == "expr") param.fragSpec = MacroFragSpec::Expr;
            else if (fragTok.text == "stmt") param.fragSpec = MacroFragSpec::Stmt;
            else if (fragTok.text == "ident") param.fragSpec = MacroFragSpec::Ident;
            else if (fragTok.text == "ty") param.fragSpec = MacroFragSpec::Ty;
            else if (fragTok.text == "item") param.fragSpec = MacroFragSpec::Item;
            else if (fragTok.text == "block") param.fragSpec = MacroFragSpec::Block;
            else if (fragTok.text == "pat") param.fragSpec = MacroFragSpec::Pat;
            else {
                diag.error(param.loc, "Unknown macro fragment specifier '" + std::string(fragTok.text) + "'");
            }
            if (match(TokenType::DOT_DOT_DOT)) {
                param.isVariadic = true;
            }
            node->params.push_back(std::move(param));
        } while (match(TokenType::COMMA));
    }
    consume(TokenType::R_PAREN, "Expected ')' after macro parameters");
    node->body = parseBlockStatement();
    
    // Macro body ends exactly after the block '}'
    node->endLoc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset + prev.text.length());
    return node;
}

std::unique_ptr<DeclNode> Parser::parseStructDecl() {
    auto node = std::make_unique<StructDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_STRUCT, "Expected 'struct'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected struct name");
    node->name = nameTok.text;
    
    node->genericParams = parseGenericParams();
    
    consume(TokenType::L_BRACE, "Expected '{' for struct body");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto field = std::make_unique<StructFieldNode>();
        field->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        
        if (match(TokenType::KW_EXPORT)) {
            field->visibility = Visibility::Public;
        }
        
        Token fName = consume(TokenType::IDENTIFIER, "Expected field name");
        field->name = fName.text;
        consume(TokenType::COLON, "Expected ':' after field name");
        field->type = parseType();
        node->fields.push_back(std::move(field));
        consume(TokenType::SEMI, "Expected ';' after struct field");
    }
    consume(TokenType::R_BRACE, "Expected '}' to end struct body");
    node->endLoc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset + prev.text.length());
    return node;
}

std::unique_ptr<DeclNode> Parser::parseEnumDecl() {
    auto node = std::make_unique<EnumDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_ENUM, "Expected 'enum'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected enum name");
    node->name = nameTok.text;
    
    node->genericParams = parseGenericParams();
    
    consume(TokenType::L_BRACE, "Expected '{' for enum body");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto var = std::make_unique<EnumVariantNode>();
        var->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        Token vName = consume(TokenType::IDENTIFIER, "Expected variant name");
        var->name = vName.text;
        
        if (match(TokenType::L_PAREN)) {
            if (!check(TokenType::R_PAREN)) {
                do {
                    auto param = std::make_unique<ParamDeclNode>();
                    param->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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
    node->endLoc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset + prev.text.length());
    return node;
}

std::unique_ptr<DeclNode> Parser::parseTraitDecl() {
    auto node = std::make_unique<TraitDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_TRAIT, "Expected 'trait'");
    Token nameTok = consume(TokenType::IDENTIFIER, "Expected trait name");
    node->name = nameTok.text;
    
    node->genericParams = parseGenericParams();

    consume(TokenType::L_BRACE, "Expected '{'");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto annots = parseAnnotations();
        Visibility visibility = Visibility::Public;
        if (match(TokenType::KW_EXPORT)) {
            // Optional export keyword, ignored since trait items are always public
        }

        if (match(TokenType::KW_TYPE)) {
            auto atNode = std::make_unique<TypeAliasDeclNode>();
            atNode->loc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset);
            atNode->name = consume(TokenType::IDENTIFIER, "Expected associated type name").text;
            atNode->genericParams = parseGenericParams();
            if (match(TokenType::COLON)) {
                do {
                    if (current.type == TokenType::LIFETIME) {
                        auto ltNode = std::make_unique<LifetimeNode>();
                        ltNode->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                        ltNode->name = current.text;
                        advance();
                        atNode->bounds.push_back(std::move(ltNode));
                    } else {
                        atNode->bounds.push_back(parseNamedType());
                    }
                } while (match(TokenType::PLUS));
            }
            consume(TokenType::SEMI, "Expected ';' after associated type declaration");
            atNode->visibility = visibility;
            atNode->annotations = std::move(annots);
            node->associatedTypes.push_back(std::move(atNode));
            continue;
        }

        auto decl = parseFunctionDecl(true); // Traits have empty bodies
        decl->visibility = visibility;
        decl->annotations = std::move(annots);

        auto funcDecl = std::unique_ptr<FunctionDeclNode>(dynamic_cast<FunctionDeclNode*>(decl.release()));
        if (funcDecl) node->methods.push_back(std::move(funcDecl));
        else {
            diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected function in trait");
            throw ParseError();
        }
    }
    consume(TokenType::R_BRACE, "Expected '}'");
    node->endLoc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset + prev.text.length());
    return node;
}

std::unique_ptr<DeclNode> Parser::parseImplDecl() {
    auto node = std::make_unique<ImplDeclNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_IMPL, "Expected 'impl'");
    
    node->genericParams = parseGenericParams();
    
    node->selfType = parseType();
    if (match(TokenType::KW_FOR)) {
        node->traitType = std::move(node->selfType);
        node->selfType = parseType();
    }
    
    consume(TokenType::L_BRACE, "Expected '{'");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        auto annots = parseAnnotations();
        Visibility visibility = (node->traitType != nullptr || match(TokenType::KW_EXPORT)) ? Visibility::Public : Visibility::Internal;
        
        if (match(TokenType::KW_TYPE)) {
            auto atNode = std::make_unique<TypeAliasDeclNode>();
            atNode->loc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset);
            atNode->name = consume(TokenType::IDENTIFIER, "Expected associated type name").text;
            atNode->genericParams = parseGenericParams();
            consume(TokenType::EQUAL, "Expected '=' in associated type implementation");
            atNode->aliasedType = parseType();
            consume(TokenType::SEMI, "Expected ';' after associated type implementation");
            atNode->visibility = visibility;
            atNode->annotations = std::move(annots);
            node->associatedTypes.push_back(std::move(atNode));
            continue;
        }

        auto decl = parseFunctionDecl();
        decl->visibility = visibility;
        decl->annotations = std::move(annots);
        
        auto funcDecl = std::unique_ptr<FunctionDeclNode>(dynamic_cast<FunctionDeclNode*>(decl.release()));
        if (funcDecl) node->methods.push_back(std::move(funcDecl));
        else {
            diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected function in impl block");
            throw ParseError();
        }
    }
    consume(TokenType::R_BRACE, "Expected '}'");
    node->endLoc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset + prev.text.length());
    return node;
}

// ==========================================
// STATEMENTS
// ==========================================

std::unique_ptr<StmtNode> Parser::parseStatement() {
    std::string label = "";
    if (check(TokenType::LIFETIME) && peek.type == TokenType::COLON) {
        label = current.text;
        advance(); // consume label
        advance(); // consume colon
    }

    std::unique_ptr<StmtNode> stmt;
    if (check(TokenType::KW_IF)) stmt = parseIfStatement();
    else if (check(TokenType::KW_WHILE)) stmt = parseWhileStatement();
    else if (check(TokenType::KW_FOR)) stmt = parseForStatement();
    else if (check(TokenType::KW_RETURN)) stmt = parseReturnStatement();
    else if (check(TokenType::KW_BREAK)) stmt = parseBreakStatement();
    else if (check(TokenType::KW_CONTINUE)) stmt = parseContinueStatement();
    else if (check(TokenType::KW_PRINT)) stmt = parsePrintStatement();
    else if (check(TokenType::L_BRACE)) stmt = parseBlockStatement();
    else if (check(TokenType::KW_UNSAFE)) stmt = parseUnsafeStatement();
    else if (check(TokenType::KW_COMPTIME)) stmt = parseComptimeStatement();
    else stmt = parseExpressionStatement();

    if (!label.empty()) {
        if (auto* w = dynamic_cast<WhileStmtNode*>(stmt.get())) w->label = label;
        else if (auto* f = dynamic_cast<ForStmtNode*>(stmt.get())) f->label = label;
        else {
            diag.error(stmt->loc, "Label can only be applied to loops");
            throw ParseError();
        }
    }
    return stmt;
}

std::unique_ptr<StmtNode> Parser::parseAssignmentStatement() {
    diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "parseAssignmentStatement not implemented");
    throw ParseError();
}

std::unique_ptr<StmtNode> Parser::parseExpressionStatement() {
    auto expr = parseExpression();
    if (current.type == TokenType::R_BRACE) {
        auto stmt = std::make_unique<ExprStmtNode>();
        stmt->expr = std::move(expr);
        stmt->hasSemicolon = false;
        return stmt;
    }
    consume(TokenType::SEMI, "Expected ';' after expression");
    auto stmt = std::make_unique<ExprStmtNode>();
    stmt->expr = std::move(expr);
    stmt->hasSemicolon = true;
    return stmt;
}

std::unique_ptr<BlockStmtNode> Parser::parseBlockStatement() {
    auto block = std::make_unique<BlockStmtNode>();
    block->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::L_BRACE, "Expected '{' to start block");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        block->body.push_back(parseDeclaration());
    }
    
    // Check if the last item is an expression statement without a semicolon
    if (!block->body.empty()) {
        if (auto* exprStmt = dynamic_cast<ExprStmtNode*>(block->body.back().get())) {
            // Because ExprStmtNode might have been created without a semicolon if it was right before '}'
            // We can check if it's right before R_BRACE. We actually handle this in parseExpressionStatement!
            // Wait, parseExpressionStatement already checks for R_BRACE and skips SEMI.
            // Let's just extract it if it's an ExprStmtNode and we are at R_BRACE.
            // Actually, we need to know if it HAD a semicolon.
            // Let's modify parseDeclaration/parseExpressionStatement to mark if it had a semicolon, 
            // or just rely on the fact that if it's the last statement and it's an ExprStmtNode, it's a tail expr.
            // In Rust, a block returns the last expression IF there is no semicolon.
            // We can just pop it and set as tailExpr.
        }
    }
    
    consume(TokenType::R_BRACE, "Expected '}' to end block");
    
    // Convert last ExprStmtNode to tailExpr if it didn't end with a semicolon.
    if (!block->body.empty()) {
        auto* lastItem = block->body.back().get();
        if (auto* exprStmt = dynamic_cast<ExprStmtNode*>(lastItem)) {
            if (!exprStmt->hasSemicolon) {
                block->tailExpr = std::move(exprStmt->expr);
                block->body.pop_back();
            }
        }
    }

    return block;
}

std::unique_ptr<StmtNode> Parser::parseIfStatement() {
    auto ifStmt = std::make_unique<IfStmtNode>();
    ifStmt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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
    whileStmt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_WHILE, "Expected 'while'");
    whileStmt->condition = parseExpression(false); // No struct literals
    whileStmt->body = parseBlockStatement();
    return whileStmt;
}

std::unique_ptr<StmtNode> Parser::parseForStatement() {
    SourceLocation loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_FOR, "Expected 'for'");
    
    if (match(TokenType::AT)) {
        auto mFor = std::make_unique<MacroExpandForStmt>();
        mFor->loc = loc;
        mFor->iterName = consume(TokenType::IDENTIFIER, "Expected identifier after '@'").text;
        consume(TokenType::KW_IN, "Expected 'in'");
        consume(TokenType::AT, "Expected '@' before list name");
        mFor->listName = consume(TokenType::IDENTIFIER, "Expected identifier after '@'").text;
        mFor->body = parseBlockStatement();
        return mFor;
    }

    auto forStmt = std::make_unique<ForStmtNode>();
    forStmt->loc = loc;
    consume(TokenType::L_PAREN, "Expected '(' after 'for'");

    if (current.type == TokenType::KW_DEC || current.type == TokenType::KW_CONST || current.type == TokenType::SEMI) {
        forStmt->kind = ForKind::CStyle;
        if (current.type != TokenType::SEMI) {
            forStmt->init = parseDeclaration();
        } else {
            advance();
        }
        
        if (current.type != TokenType::SEMI) {
            forStmt->cond = parseExpression(false);
        }
        consume(TokenType::SEMI, "Expected ';' after for condition");
        
        if (current.type != TokenType::R_PAREN) {
            forStmt->step = parseExpression(false);
        }
        consume(TokenType::R_PAREN, "Expected ')' after for update");
    } else {
        forStmt->pattern = parsePattern();
        // Fallback for bindingName
        if (auto bindPat = dynamic_cast<fl::IdentifierPatternNode*>(forStmt->pattern.get())) {
            forStmt->bindingName = bindPat->segments.back();
        } else {
            forStmt->bindingName = "<pattern>";
        }
        consume(TokenType::KW_IN, "Expected 'in' after loop pattern");
        forStmt->iterable = parseExpression(true);
        consume(TokenType::R_PAREN, "Expected ')' after for condition");
        forStmt->kind = ForKind::ForEach;
    }

    forStmt->body = parseBlockStatement();
    return forStmt;
}

std::unique_ptr<StmtNode> Parser::parseReturnStatement() {
    auto retStmt = std::make_unique<ReturnStmtNode>();
    retStmt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_RETURN, "Expected 'return'");
    if (!check(TokenType::SEMI)) retStmt->value = parseExpression();
    consume(TokenType::SEMI, "Expected ';' after return value");
    return retStmt;
}

std::unique_ptr<StmtNode> Parser::parseBreakStatement() {
    auto stmt = std::make_unique<BreakStmtNode>();
    stmt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_BREAK, "Expected 'break'");
    if (current.type == TokenType::LIFETIME) {
        stmt->label = current.text;
        advance();
    }
    consume(TokenType::SEMI, "Expected ';' after break");
    return stmt;
}

std::unique_ptr<StmtNode> Parser::parseContinueStatement() {
    auto stmt = std::make_unique<ContinueStmtNode>();
    stmt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_CONTINUE, "Expected 'continue'");
    if (current.type == TokenType::LIFETIME) {
        stmt->label = current.text;
        advance();
    }
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

std::unique_ptr<StmtNode> Parser::parseUnsafeStatement() {
    auto stmt = std::make_unique<UnsafeStmtNode>();
    stmt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_UNSAFE, "Expected 'unsafe'");
    stmt->body = parseBlockStatement();
    return stmt;
}

std::unique_ptr<StmtNode> Parser::parseComptimeStatement() {
    auto stmt = std::make_unique<ComptimeStmtNode>();
    stmt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_COMPTIME, "Expected 'comptime'");
    stmt->body = parseBlockStatement();
    return stmt;
}

// ==========================================
// TYPE PARSING
// ==========================================

std::unique_ptr<TypeNode> Parser::parseType() {
    bool isUnsafe = false;
    if (match(TokenType::KW_UNSAFE)) {
        isUnsafe = true;
    }
    
    if (match(TokenType::KW_FN)) {
        auto node = std::make_unique<FunctionTypeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        node->isUnsafe = isUnsafe;
        consume(TokenType::L_PAREN, "Expected '(' in function type parameters");
        if (!check(TokenType::R_PAREN)) {
            do {
                node->params.push_back(parseType());
            } while (match(TokenType::COMMA));
        }
        consume(TokenType::R_PAREN, "Expected ')' after function type parameters");
        if (match(TokenType::ARROW)) {
            node->returnType = parseType();
        }
        return node;
    }
    
    if (isUnsafe) {
        diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected 'fn' after 'unsafe' in function type");
        throw ParseError();
    }
    
    if (current.type == TokenType::LIFETIME) {
        auto node = std::make_unique<LifetimeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        node->name = current.text;
        advance();
        return node;
    }
    if (match(TokenType::BANG)) {
        auto node = std::make_unique<NeverTypeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        return node;
    }
    if (match(TokenType::KW_DYN)) {
        auto node = std::make_unique<TraitObjectTypeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        node->trait = parseNamedType();
        return node;
    }
    if (match(TokenType::MULTIPLY)) {
        auto node = std::make_unique<PointerTypeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        
        node->isMutable = false; // default is immutable
        if (match(TokenType::KW_RW)) {
            node->isMutable = true;
        } else if (match(TokenType::KW_CONST)) {
            node->isMutable = false;
        }
        
        node->inner = parseType();
        return node;
    }
    if (match(TokenType::BIT_AND)) {
        auto node = std::make_unique<ReferenceTypeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        if (current.type == TokenType::LIFETIME) {
            auto lt = std::make_unique<LifetimeNode>();
            lt->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            lt->name = current.text;
            advance();
            node->lifetime = std::move(lt);
        }
        node->isMutable = match(TokenType::KW_RW);
        node->inner = parseType();
        return node;
    }
    if (match(TokenType::L_BRACKET)) {
        auto node = std::make_unique<ArrayTypeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        node->elementType = parseType();
        if (match(TokenType::COMMA)) {
            node->size = parseExpression(true);
            std::cout << "[DEBUG] Parser parsed array size, size ptr is " << node->size.get() << "\n";
        } else if (match(TokenType::SEMI)) {
            node->size = parseExpression(true);
            std::cout << "[DEBUG] Parser parsed array size with semicolon, size ptr is " << node->size.get() << "\n";
        }
        consume(TokenType::R_BRACKET, "Expected ']' after array type");
        return node;
    }
    if (match(TokenType::L_PAREN)) {
        auto node = std::make_unique<TupleTypeNode>();
        node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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
        type->loc = SourceLocation::fromLineCol(fileId, tok.line, tok.col, tok.byteOffset);
        type->kind = tok.builtinKind;
        return type;
    }
    if (check(TokenType::IDENTIFIER) || check(TokenType::KW_SELF_TYP)) return parseNamedType();
    
    extern bool g_quiet; if (!g_quiet) std::cerr << "[DEBUG] parseType failed at byte " << current.byteOffset << " token " << (int)current.type << " text: " << std::string(current.text) << "\n";
    diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected a type");
    throw ParseError();
}

std::unique_ptr<TypeNode> Parser::parseNamedType() {
    auto node = std::make_unique<NamedTypeNode>();
    node->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    
    do {
        Token idTok = current;
        if (match(TokenType::IDENTIFIER) || match(TokenType::KW_SELF_TYP)) {
            // consumed
        } else {
            diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected identifier or Self in type path");
            throw ParseError();
        }
        node->segments.push_back(idTok.text);
        
        if (check(TokenType::GENERIC_START) || check(TokenType::LESS_THAN)) {
            auto args = parseGenericArgs(&node->associatedBindings);
            for (auto& arg : args) {
                node->genericArgs.push_back(std::move(arg));
            }
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
        expr->loc = SourceLocation::fromLineCol(fileId, op.line, op.col, op.byteOffset);
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
        if (match(TokenType::QUESTION)) {
            auto tryExpr = std::make_unique<TryExpr>();
            tryExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            tryExpr->expr = std::move(expr);
            expr = std::move(tryExpr);
            continue;
        }
        if (match(TokenType::PLUS_PLUS)) {
            auto unExpr = std::make_unique<UnaryExpr>();
            unExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            unExpr->op = UnaryOp::PostInc;
            unExpr->operand = std::move(expr);
            expr = std::move(unExpr);
            continue;
        }
        if (match(TokenType::MINUS_MINUS)) {
            auto unExpr = std::make_unique<UnaryExpr>();
            unExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            unExpr->op = UnaryOp::PostDec;
            unExpr->operand = std::move(expr);
            expr = std::move(unExpr);
            continue;
        }
        if (match(TokenType::BANG)) {
            auto macroCall = std::make_unique<MacroCallExpr>();
            macroCall->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            if (auto identExpr = dynamic_cast<IdentifierExpr*>(expr.get())) {
                if (identExpr->segments.size() == 1) {
                    macroCall->name = std::string(identExpr->segments[0]);
                } else {
                    diag.error(macroCall->loc, "Macro call must be a simple identifier");
                }
            } else {
                diag.error(macroCall->loc, "Expected identifier before '!' in macro call");
            }
            
            consume(TokenType::L_PAREN, "Expected '(' after macro '!'");
            if (!check(TokenType::R_PAREN)) {
                do {
                    MacroCallArgNode arg;
                    arg.node = parseExpression(true);
                    macroCall->args.push_back(std::move(arg));
                } while (match(TokenType::COMMA));
            }
            consume(TokenType::R_PAREN, "Expected ')' after macro arguments");
            expr = std::move(macroCall);
            continue;
        }
        if (match(TokenType::L_PAREN)) {
            // Function Call
            auto callExpr = std::make_unique<CallExpr>();
            callExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            callExpr->callee = std::move(expr);
            if (!check(TokenType::R_PAREN)) {
                do {
                    CallArgNode arg;
                    arg.loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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
            idxExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            idxExpr->base = std::move(expr);
            idxExpr->index = parseExpression(true);
            consume(TokenType::R_BRACKET, "Expected ']' after index");
            expr = std::move(idxExpr);
        } else if (match(TokenType::DOT)) {
            if (match(TokenType::KW_AWAIT)) {
                auto awaitExpr = std::make_unique<AwaitExpr>();
                awaitExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                awaitExpr->expr = std::move(expr);
                expr = std::move(awaitExpr);
                continue;
            }
            // Tuple index: t.0, t.1, t.2 ...
            if (current.type == TokenType::INTEGER_LITERAL) {
                auto tupleIdx = std::make_unique<TupleIndexExpr>();
                tupleIdx->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                tupleIdx->object = std::move(expr);
                uint32_t idx = 0;
                std::from_chars(current.text.data(),
                                current.text.data() + current.text.size(), idx);
                tupleIdx->index = idx;
                advance();
                expr = std::move(tupleIdx);
                continue;
            }
            // Member Access or Method Call
            Token memberTok;
            if (current.type == TokenType::IDENTIFIER || current.type == TokenType::KW_PRINT) {
                memberTok = current;
                advance();
            } else {
                diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected member name or tuple index");
                throw ParseError();
            }
            
            // Check for Generic Arguments like `a.foo::<I32>()`? We don't have turbofish yet, but we might.
            // Let's assume standard `a.foo()` for now.
            if (match(TokenType::L_PAREN)) {
                auto methodCall = std::make_unique<MethodCallExpr>();
                methodCall->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                methodCall->object = std::move(expr);
                methodCall->methodName = memberTok.text;
                
                if (!check(TokenType::R_PAREN)) {
                    do {
                        CallArgNode arg;
                        arg.loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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
                memExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                memExpr->object = std::move(expr);
                memExpr->member = memberTok.text;
                expr = std::move(memExpr);
            }
        } else if (allowStructLiteral && check(TokenType::L_BRACE)) {
            // Struct Initialization
            // This is only valid if `expr` is an IdentifierExpr representing a path.
            if (auto idExpr = dynamic_cast<IdentifierExpr*>(expr.get())) {
                auto structInit = std::make_unique<StructInitExpr>();
                structInit->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
                structInit->path = idExpr->segments;
                structInit->genericArgs = std::move(idExpr->genericArgs);
                
                consume(TokenType::L_BRACE, "");
                while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
                    FieldInitNode fieldInit;
                    fieldInit.loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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
        } else if (match(TokenType::QUESTION)) {
            // Postfix `?` — Try operator
            auto tryExpr = std::make_unique<TryExpr>();
            tryExpr->loc = SourceLocation::fromLineCol(fileId, prev.line, prev.col, prev.byteOffset);
            tryExpr->expr = std::move(expr);
            expr = std::move(tryExpr);
        } else {
            break;
        }
    }
    
    return expr;
}

std::unique_ptr<ExprNode> Parser::parseValuePath() {
    auto ident = std::make_unique<IdentifierExpr>();
    ident->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    do {
        Token idTok;
        if (match(TokenType::KW_SELF_TYP)) {
            idTok = prev;
        } else if (match(TokenType::KW_SELF_VAL)) {
            idTok = prev;
        } else if (current.type == TokenType::IDENTIFIER || current.type == TokenType::KW_PRINT) {
            idTok = current;
            advance();
        } else {
            extern bool g_quiet; if (!g_quiet) std::cerr << "[DEBUG] parseValuePath failed at byte " << current.byteOffset << " token " << (int)current.type << " text: " << std::string(current.text) << "\n";
            diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected identifier in value path");
            throw ParseError();
        }
        ident->segments.push_back(idTok.text);
        
        if (check(TokenType::GENERIC_START)) {
            auto args = parseGenericArgs();
            for (auto& arg : args) {
                ident->genericArgs.push_back(std::move(arg));
            }
        }
    } while (match(TokenType::COLON_COLON));
    
    return ident;
}

std::unique_ptr<ExprNode> Parser::parseMatchExpr() {
    auto matchExpr = std::make_unique<MatchExpr>();
    matchExpr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    consume(TokenType::KW_MATCH, "Expected 'match'");
    matchExpr->subject = parseExpression(false); // No struct literals in match subject
    
    consume(TokenType::L_BRACE, "Expected '{' for match body");
    while (!check(TokenType::R_BRACE) && !check(TokenType::END_OF_FILE)) {
        MatchArmNode arm;
        arm.loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
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
            match(TokenType::COMMA); // Optional trailing comma
        }
        matchExpr->arms.push_back(std::move(arm));
    }
    consume(TokenType::R_BRACE, "Expected '}'");
    return matchExpr;
}

std::unique_ptr<PatternNode> Parser::parsePattern() {

    if (check(TokenType::L_PAREN)) {
        consume(TokenType::L_PAREN, "");
        auto tuplePat = std::make_unique<TuplePatternNode>();
        if (!check(TokenType::R_PAREN)) {
            do {
                if (match(TokenType::DOT_DOT)) {
                    tuplePat->hasRest = true;
                    break;
                }
                tuplePat->elements.push_back(parsePattern());
            } while (match(TokenType::COMMA));
        }
        consume(TokenType::R_PAREN, "Expected ')' for tuple pattern");
        return tuplePat;
    }
    
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
            if (pat->segments.size() > 1 || check(TokenType::L_PAREN) || check(TokenType::L_BRACE)) {
                if (check(TokenType::L_BRACE)) {
                    auto structPat = std::make_unique<StructPatternNode>();
                    structPat->path = std::move(pat->segments);
                    consume(TokenType::L_BRACE, "");
                    if (!check(TokenType::R_BRACE)) {
                        do {
                            if (match(TokenType::DOT_DOT)) {
                                structPat->hasRest = true;
                                break;
                            }
                            StructPatternField field;
                            Token fieldName = consume(TokenType::IDENTIFIER, "Expected field name in struct pattern");
                            field.name = fieldName.text;
                            if (match(TokenType::COLON)) {
                                field.pattern = parsePattern();
                            } else {
                                auto idPat = std::make_unique<IdentifierPatternNode>();
                                idPat->segments.push_back(field.name);
                                field.pattern = std::move(idPat);
                            }
                            structPat->fields.push_back(std::move(field));
                        } while (match(TokenType::COMMA));
                    }
                    consume(TokenType::R_BRACE, "Expected '}' after struct pattern");
                    return structPat;
                } else {
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
            }
            return pat;
        }
    }
    
    if (check(TokenType::INTEGER_LITERAL) || check(TokenType::FLOAT_LITERAL) || check(TokenType::STRING_LITERAL) || check(TokenType::KW_TRUE) || check(TokenType::KW_FALSE)) {
        auto litPat = std::make_unique<LiteralPatternNode>();
        litPat->lit = std::unique_ptr<LiteralExpr>(static_cast<LiteralExpr*>(parsePrimary(true).release()));
        return litPat;
    }
    
    diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected a pattern (e.g., literal, identifier, or '_') in match arm");
    throw ParseError();
}

std::unique_ptr<ExprNode> Parser::parseLambdaExpr() {
    auto lambda = std::make_unique<LambdaExpr>();
    lambda->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
    if (match(TokenType::KW_MOVE)) {
        lambda->isMove = true;
    }
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
    consume(TokenType::ARROW, "Expected '->' after lambda parameters");
    
    ParserState bookmark = saveState();
    bool parsedTypeAndBlock = false;
    
    try {
        lambda->returnType = parseType();
        if (check(TokenType::L_BRACE)) {
            lambda->body = parseBlockStatement();
            parsedTypeAndBlock = true;
        } else {
            restoreState(bookmark);
        }
    } catch (const ParseError&) {
        restoreState(bookmark);
    }
    
    if (!parsedTypeAndBlock) {
        auto expr = parseExpression(true);
        auto block = std::make_unique<BlockStmtNode>();
        block->tailExpr = std::move(expr);
        lambda->body = std::move(block);
    }
    return lambda;
}

std::unique_ptr<ExprNode> Parser::parsePrimary(bool allowStructLiteral) {
    if (check(TokenType::INTEGER_LITERAL)) { std::string_view text = current.text; advance();
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Integer; lit->rawText = text;
        return lit;
    }
    if (check(TokenType::FLOAT_LITERAL)) { std::string_view text = current.text; advance();
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Float; lit->rawText = text;
        return lit;
    }
    if (check(TokenType::STRING_LITERAL)) { std::string_view text = current.text; advance();
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Str; lit->rawText = text;
        return lit;
    }
    if (check(TokenType::RAW_STRING_LITERAL)) { std::string_view text = current.text; advance();
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::RawStr;
        return lit;
    }
    if (match(TokenType::BYTE_LITERAL)) {
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Byte;
        return lit;
    }
    if (match(TokenType::AT)) {
        Token ident = consume(TokenType::IDENTIFIER, "Expected identifier after '@'");
        auto ph = std::make_unique<PlaceholderExpr>();
        ph->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        ph->data.name = ident.text;
        return ph;
    }
    if (match(TokenType::BYTE_STRING_LITERAL)) {
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::ByteStr;
        return lit;
    }
    if (check(TokenType::CHAR_LITERAL)) { std::string_view text = current.text; advance();
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Char; lit->rawText = text;
        return lit;
    }
    if (check(TokenType::KW_TRUE) || check(TokenType::KW_FALSE)) { std::string_view text = current.text; advance();
        auto lit = std::make_unique<LiteralExpr>();
        lit->kind = LiteralKind::Bool; lit->rawText = text;
        return lit;
    }
    if (check(TokenType::IDENTIFIER) || check(TokenType::KW_SELF_VAL)) {
        return parseValuePath();
    }
    if (check(TokenType::KW_MATCH)) {
        return parseMatchExpr();
    }
    if (check(TokenType::KW_MOVE) || check(TokenType::BIT_OR)) {
        return parseLambdaExpr();
    }
    if (match(TokenType::L_BRACKET)) {
        auto arr = std::make_unique<ArrayLiteralExpr>();
        arr->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        if (!check(TokenType::R_BRACKET)) {
            do {
                if (check(TokenType::DOT_DOT_DOT)) {
                    // Spread/variadic placeholder - skip for now
                    advance();
                    break;
                }
                arr->elements.push_back(parseExpression(true));
            } while (match(TokenType::COMMA) && !check(TokenType::R_BRACKET));
        }
        consume(TokenType::R_BRACKET, "Expected ']' after array literal");
        return arr;
    }
    if (match(TokenType::L_PAREN)) {
        if (match(TokenType::R_PAREN)) {
            auto tup = std::make_unique<TupleLiteralExpr>();
            tup->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            return tup;
        }
        auto expr = parseExpression(true);
        if (match(TokenType::COMMA)) {
            auto tup = std::make_unique<TupleLiteralExpr>();
            tup->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
            tup->elements.push_back(std::move(expr));
            if (!check(TokenType::R_PAREN)) {
                do {
                    tup->elements.push_back(parseExpression(true));
                } while (match(TokenType::COMMA) && !check(TokenType::R_PAREN));
            }
            consume(TokenType::R_PAREN, "Expected ')' after tuple elements");
            return tup;
        }
        consume(TokenType::R_PAREN, "Expected ')' after expression");
        return expr;
    }
    if (match(TokenType::KW_SIZEOF)) {
        auto soe = std::make_unique<SizeofExpr>();
        soe->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        consume(TokenType::L_PAREN, "Expected '(' after sizeof");
        soe->targetType = parseType();
        consume(TokenType::R_PAREN, "Expected ')' after sizeof type");
        return soe;
    }
    if (match(TokenType::KW_ALIGNOF)) {
        auto aoe = std::make_unique<AlignofExpr>();
        aoe->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        consume(TokenType::L_PAREN, "Expected '(' after alignof");
        aoe->targetType = parseType();
        consume(TokenType::R_PAREN, "Expected ')' after alignof type");
        return aoe;
    }
    if (match(TokenType::KW_TYPEOF)) {
        auto toe = std::make_unique<TypeofExpr>();
        toe->loc = SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset);
        consume(TokenType::L_PAREN, "Expected '(' after typeof");
        toe->expr = parseExpression();
        consume(TokenType::R_PAREN, "Expected ')' after typeof expression");
        return toe;
    }

    diag.error(SourceLocation::fromLineCol(fileId, current.line, current.col, current.byteOffset), "Expected expression (e.g., literal, identifier, or '('). Got token " + std::to_string((int)current.type) + " text: " + std::string(current.text));
    std::cout << "[DEBUG] Expected expression at line " << current.line << " col " << current.col << " token type " << (int)current.type << " text: " << current.text << "\n";
    throw ParseError();
}

} // namespace fl