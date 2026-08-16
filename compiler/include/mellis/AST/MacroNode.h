#pragma once

#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include <vector>
#include <string>
#include <memory>

#include "mellis/AST/PlaceholderData.h"
#include "mellis/Core/Types.h"

namespace fl {

class MacroParamNode {
public:
    std::string name;
    MacroFragSpec fragSpec;
    bool isVariadic = false;
    SourceLocation loc;
};

class MacroDeclNode : public DeclNode {
public:
    std::string name;
    std::vector<MacroParamNode> params;
    std::unique_ptr<BlockStmtNode> body;

    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class MacroCallArgNode {
public:
    std::unique_ptr<ASTNode> node; // Could be ExprNode, TypeNode, BlockStmtNode, etc.
};

class MacroCallExpr : public ExprNode {
public:
    std::string name;
    std::vector<MacroCallArgNode> args;
    MacroID resolvedMacroId = kInvalidMacroID;

    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class MacroCallStmt : public StmtNode {
public:
    std::string name;
    std::vector<MacroCallArgNode> args;
    MacroID resolvedMacroId = kInvalidMacroID;

    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class MacroExpandForStmt : public StmtNode {
public:
    std::string iterName;
    std::string listName;
    std::unique_ptr<BlockStmtNode> body;
    MacroID resolvedMacroId = kInvalidMacroID; // if necessary

    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

} // namespace fl
