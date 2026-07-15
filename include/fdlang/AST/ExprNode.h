#pragma once
#include "ASTNode.h"
#include <string_view>

namespace fl {
    class ExprNode : public ASTNode {};

    class NumberExpr : public ExprNode {
    public:
        std::string_view value;
        explicit NumberExpr(std::string_view val) : value(val) {}
    };

    class IdentifierExpr : public ExprNode {
    public:
        std::string_view name;
        explicit IdentifierExpr(std::string_view n) : name(n) {}
    };
}