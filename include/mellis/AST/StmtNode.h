#pragma once
#include "mellis/AST/ASTNode.h"
#include "mellis/Core/Types.h"
#include "mellis/AST/PlaceholderData.h"
#include <vector>
#include <memory>
#include <string_view>

namespace fl {

class ExprNode;

class StmtNode : public ItemNode {};

class PlaceholderStmt : public StmtNode {
public:
    PlaceholderData data;

    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class BlockStmtNode : public StmtNode {
public:
    std::vector<std::unique_ptr<ItemNode>> body;
    ScopeID bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class ExprStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode> expr;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class IfStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode>      condition;
    std::unique_ptr<BlockStmtNode> thenBranch;
    std::unique_ptr<StmtNode>      elseBranch;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class WhileStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode>      condition;
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
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
    ASTNode* cloneImpl() const override;
};

class ReturnStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode> value; // nullptr if return void
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class BreakStmtNode : public StmtNode {
public:
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class ContinueStmtNode : public StmtNode {
public:
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class UnsafeStmtNode : public StmtNode {
public:
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class ComptimeStmtNode : public StmtNode {
public:
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

} // namespace fl