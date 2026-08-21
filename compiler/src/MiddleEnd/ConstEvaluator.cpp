#include "mellis/MiddleEnd/ConstEvaluator.h"
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/FrontEnd/Token.h"
#include "mellis/Core/FLType.h"
#include <string>

namespace fl {

std::optional<int64_t> ConstEvaluator::evaluate(ExprNode* expr, SymbolTable* symbolTable) {
    if (!expr) return std::nullopt;
    
    std::cout << "[DEBUG] ConstEvaluator evaluating expr of type: " << typeid(*expr).name() << "\n";

    // 1. Trường hợp là số nguyên trực tiếp (Integer Literal)
    if (auto* lit = dynamic_cast<LiteralExpr*>(expr)) {
        std::cout << "[DEBUG] ConstEvaluator LiteralExpr kind: " << static_cast<int>(lit->kind) << "\n";
        if (lit->kind == LiteralKind::Integer) {
            // 1. If we have rawText, parse it directly (Parser only sets rawText)
            if (!lit->rawText.empty()) {
                try {
                    int64_t val = static_cast<int64_t>(std::stoull(std::string(lit->rawText)));
                    return val;
                } catch (const std::exception& e) {
                }
            }
            // 2. Fallback to variant value if rawText is not available or parsing failed
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

    if (auto* sizeExpr = dynamic_cast<SizeofExpr*>(expr)) {
        if (sizeExpr->evaluatedTargetType) {
            // Simplified size calculation for demonstration
            // Real compiler would use a LayoutEngine or TargetData
            if (auto* prim = dynamic_cast<const PrimitiveType*>(sizeExpr->evaluatedTargetType)) {
                switch (prim->builtinKind) {
                    case BuiltinKind::I8: case BuiltinKind::U8: case BuiltinKind::Bool: return 1;
                    case BuiltinKind::I16: case BuiltinKind::U16: return 2;
                    case BuiltinKind::I32: case BuiltinKind::U32: case BuiltinKind::F32: return 4;
                    case BuiltinKind::I64: case BuiltinKind::U64: case BuiltinKind::F64: return 8;
                    default: return 0;
                }
            } else if (dynamic_cast<const PointerType*>(sizeExpr->evaluatedTargetType)) {
                return 8; // Assuming 64-bit platform
            } else if (dynamic_cast<const ReferenceType*>(sizeExpr->evaluatedTargetType)) {
                return 8;
            }
        }
    }

    if (auto* alignExpr = dynamic_cast<AlignofExpr*>(expr)) {
        if (alignExpr->evaluatedTargetType) {
            if (auto* prim = dynamic_cast<const PrimitiveType*>(alignExpr->evaluatedTargetType)) {
                switch (prim->builtinKind) {
                    case BuiltinKind::I8: case BuiltinKind::U8: case BuiltinKind::Bool: return 1;
                    case BuiltinKind::I16: case BuiltinKind::U16: return 2;
                    case BuiltinKind::I32: case BuiltinKind::U32: case BuiltinKind::F32: return 4;
                    case BuiltinKind::I64: case BuiltinKind::U64: case BuiltinKind::F64: return 8;
                    default: return 1;
                }
            } else if (dynamic_cast<const PointerType*>(alignExpr->evaluatedTargetType)) {
                return 8;
            } else if (dynamic_cast<const ReferenceType*>(alignExpr->evaluatedTargetType)) {
                return 8;
            }
        }
    }

    return std::nullopt; // Không thể đánh giá tại compile-time
}

} // namespace fl
