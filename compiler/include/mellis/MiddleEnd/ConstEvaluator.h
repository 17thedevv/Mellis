#pragma once
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include <optional>

namespace fl {

class ConstEvaluator {
public:
    // Trả về optional<int64_t> để biết biểu thức có thực sự là hằng số compile-time hay không
    static std::optional<int64_t> evaluate(ExprNode* expr, SymbolTable* symbolTable = nullptr);
};

} // namespace fl
