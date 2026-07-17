#pragma once
#include "fdlang/AST/ASTNode.h"
#include "fdlang/Core/Types.h"
#include <vector>
#include <memory>
#include <string_view>

namespace fl {

class ExprNode;

class StmtNode : public ItemNode {};

class BlockStmtNode : public StmtNode {
public:
    std::vector<std::unique_ptr<ItemNode>> body;
    ScopeID                                bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class ExprStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode> expr;
    void accept(ASTVisitor& v) override;
};

class IfStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode>      condition;
    std::unique_ptr<BlockStmtNode> thenBranch;
    std::unique_ptr<StmtNode>      elseBranch;
    void accept(ASTVisitor& v) override;
};

class WhileStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode>      condition;
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
};

enum class ForKind : uint8_t { ForEach, CStyle };

class ForStmtNode : public StmtNode {
public:
    ForKind kind;
    std::string_view               bindingName;
    SymbolID                       bindingId = kInvalidSymbolID;
    std::unique_ptr<ExprNode>      iterable;
    
    std::unique_ptr<ItemNode>      init;
    std::unique_ptr<ExprNode>      cond;
    std::unique_ptr<ExprNode>      step;
    
    std::unique_ptr<BlockStmtNode> body;
    ScopeID                        bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class ReturnStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode> value;
    void accept(ASTVisitor& v) override;
};

class BreakStmtNode : public StmtNode {
public:
    void accept(ASTVisitor& v) override;
};

class ContinueStmtNode : public StmtNode {
public:
    void accept(ASTVisitor& v) override;
};

class UnsafeStmtNode : public StmtNode {
public:
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
};

class ComptimeStmtNode : public StmtNode {
public:
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
};

} // namespace fl