#include "mellis/MiddleEnd/OperatorRegistry.h"

#include "mellis/MiddleEnd/OperatorRegistry.h"
#include "mellis/AST/ExprNode.h"

namespace fl {

TokenType mapBinaryOpToTokenType(BinaryOp op) {
    switch (op) {
        case BinaryOp::Add: return TokenType::PLUS;
        case BinaryOp::Sub: return TokenType::MINUS;
        case BinaryOp::Mul: return TokenType::MULTIPLY;
        case BinaryOp::Div: return TokenType::DIVIDE;
        case BinaryOp::Mod: return TokenType::MODULO;
        case BinaryOp::Eq: return TokenType::EQUAL_EQUAL;
        case BinaryOp::Ne: return TokenType::NOT_EQUAL;
        case BinaryOp::Lt: return TokenType::LESS_THAN;
        case BinaryOp::Le: return TokenType::LESS_THAN_EQUAL;
        case BinaryOp::Gt: return TokenType::GREATER_THAN;
        case BinaryOp::Ge: return TokenType::GREATER_THAN_EQUAL;
        case BinaryOp::LogicAnd: return TokenType::LOGICAL_AND;
        case BinaryOp::LogicOr: return TokenType::LOGICAL_OR;
        case BinaryOp::BitAnd: return TokenType::BIT_AND;
        case BinaryOp::BitOr: return TokenType::BIT_OR;
        case BinaryOp::BitXor: return TokenType::BIT_XOR;
        case BinaryOp::LShift: return TokenType::LSHIFT;
        case BinaryOp::RShift: return TokenType::RSHIFT;
        default: return TokenType::ERROR;
    }
}

TokenType mapUnaryOpToTokenType(UnaryOp op) {
    switch (op) {
        case UnaryOp::Neg: return TokenType::MINUS;
        case UnaryOp::BitNot: return TokenType::BIT_NOT;
        case UnaryOp::Not: return TokenType::BANG;
        default: return TokenType::ERROR;
    }
}

std::optional<OperatorTraitInfo> OperatorRegistry::getBinaryOperatorTrait(TokenType op) {
    switch (op) {
        case TokenType::PLUS: return OperatorTraitInfo{"core::ops", "Add", "add", false};
        case TokenType::MINUS: return OperatorTraitInfo{"core::ops", "Sub", "sub", false};
        case TokenType::MULTIPLY: return OperatorTraitInfo{"core::ops", "Mul", "mul", false};
        case TokenType::DIVIDE: return OperatorTraitInfo{"core::ops", "Div", "div", false};
        case TokenType::MODULO: return OperatorTraitInfo{"core::ops", "Rem", "rem", false};
        
        case TokenType::PLUS_ASSIGN: return OperatorTraitInfo{"core::ops", "AddAssign", "add_assign", true};
        case TokenType::MINUS_ASSIGN: return OperatorTraitInfo{"core::ops", "SubAssign", "sub_assign", true};
        case TokenType::STAR_ASSIGN: return OperatorTraitInfo{"core::ops", "MulAssign", "mul_assign", true};
        case TokenType::SLASH_ASSIGN: return OperatorTraitInfo{"core::ops", "DivAssign", "div_assign", true};
        case TokenType::PERC_ASSIGN: return OperatorTraitInfo{"core::ops", "RemAssign", "rem_assign", true};
        
        case TokenType::BIT_AND: return OperatorTraitInfo{"core::ops", "BitAnd", "bitand", false};
        case TokenType::BIT_OR: return OperatorTraitInfo{"core::ops", "BitOr", "bitor", false};
        case TokenType::BIT_XOR: return OperatorTraitInfo{"core::ops", "BitXor", "bitxor", false};
        case TokenType::LSHIFT: return OperatorTraitInfo{"core::ops", "Shl", "shl", false};
        case TokenType::RSHIFT: return OperatorTraitInfo{"core::ops", "Shr", "shr", false};
        
        case TokenType::BIT_AND_ASSIGN: return OperatorTraitInfo{"core::ops", "BitAndAssign", "bitand_assign", true};
        case TokenType::BIT_OR_ASSIGN: return OperatorTraitInfo{"core::ops", "BitOrAssign", "bitor_assign", true};
        case TokenType::BIT_XOR_ASSIGN: return OperatorTraitInfo{"core::ops", "BitXorAssign", "bitxor_assign", true};
        case TokenType::LSHIFT_ASSIGN: return OperatorTraitInfo{"core::ops", "ShlAssign", "shl_assign", true};
        case TokenType::RSHIFT_ASSIGN: return OperatorTraitInfo{"core::ops", "ShrAssign", "shr_assign", true};
        
        case TokenType::EQUAL_EQUAL:
        case TokenType::NOT_EQUAL:
            return OperatorTraitInfo{"core::cmp", "PartialEq", "eq", false}; // Not_equal uses eq and negates
            
        case TokenType::LESS_THAN:
        case TokenType::LESS_THAN_EQUAL:
        case TokenType::GREATER_THAN:
        case TokenType::GREATER_THAN_EQUAL:
            return OperatorTraitInfo{"core::cmp", "PartialOrd", "partial_cmp", false};
            
        default: return std::nullopt;
    }
}

std::optional<OperatorTraitInfo> OperatorRegistry::getUnaryOperatorTrait(TokenType op) {
    switch (op) {
        case TokenType::MINUS: return OperatorTraitInfo{"core::ops", "Neg", "neg", false};
        case TokenType::BIT_NOT: return OperatorTraitInfo{"core::ops", "Not", "not", false};
        default: return std::nullopt;
    }
}

OperatorTraitInfo OperatorRegistry::getIndexOperatorTrait(bool isMutable) {
    if (isMutable) {
        return OperatorTraitInfo{"core::ops", "IndexMut", "index_mut", false};
    } else {
        return OperatorTraitInfo{"core::ops", "Index", "index", false};
    }
}

} // namespace fl
