#include "mellis/MiddleEnd/ConstEvaluator.h"
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/FrontEnd/Token.h"
#include <string>

namespace fl {

std::optional<int64_t> ConstEvaluator::evaluate(ExprNode* expr, SymbolTable* symbolTable) {
    if (!expr) return std::nullopt;

    // 1. Trường hợp là số nguyên trực tiếp (Integer Literal)
    if (auto* lit = dynamic_cast<LiteralExpr*>(expr)) {
        if (lit->kind == LiteralKind::Integer) {
            if (auto* i64 = std::get_if<int64_t>(&lit->value)) {
                return *i64;
            }
            if (auto* u64 = std::get_if<uint64_t>(&lit->value)) {
                return static_cast<int64_t>(*u64);
            }
        }
        return std::nullopt;
    }

    // 2. Trường hợp là biểu thức nhị phân (+, -, *, /, %)
    if (auto* bin = dynamic_cast<BinaryExpr*>(expr)) {
        auto leftVal = evaluate(bin->left.get(), symbolTable);
        auto rightVal = evaluate(bin->right.get(), symbolTable);
        if (!leftVal || !rightVal) return std::nullopt;

        if (bin->op == BinaryOp::Add) return leftVal.value() + rightVal.value();
        if (bin->op == BinaryOp::Sub) return leftVal.value() - rightVal.value();
        if (bin->op == BinaryOp::Mul) return leftVal.value() * rightVal.value();
        if (bin->op == BinaryOp::Div) {
            if (rightVal.value() == 0) return std::nullopt; // Chia cho 0
            return leftVal.value() / rightVal.value();
        }
        if (bin->op == BinaryOp::Mod) {
            if (rightVal.value() == 0) return std::nullopt;
            return leftVal.value() % rightVal.value();
        }
    }

    return std::nullopt; // Không thể đánh giá tại compile-time
}

} // namespace fl
