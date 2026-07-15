#pragma once
#include "ASTNode.h"
#include "ExprNode.h" // Cần Expr để làm giá trị gán
#include <memory>
#include <string_view>

namespace fl {
    class StmtNode : public ASTNode {};

    class VarDeclStmt : public StmtNode {
    public:
        std::string_view varName;
        std::unique_ptr<ExprNode> initializer;

        VarDeclStmt(std::string_view name, std::unique_ptr<ExprNode> init)
            : varName(name), initializer(std::move(init)) {}
    };
}