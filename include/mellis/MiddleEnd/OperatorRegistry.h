#pragma once
#include "mellis/FrontEnd/Token.h"
#include <string>
#include <optional>

namespace fl {

struct OperatorTraitInfo {
    std::string traitModule; // e.g., "core::ops"
    std::string traitName;   // e.g., "Add"
    std::string methodName;  // e.g., "add"
    bool isAssign;           // e.g., true for "+="
};

class ExprNode;
enum class BinaryOp : uint8_t;
enum class UnaryOp : uint8_t;

TokenType mapBinaryOpToTokenType(BinaryOp op);
TokenType mapUnaryOpToTokenType(UnaryOp op);

class OperatorRegistry {
public:
    // Lookup the trait mapping for a given binary operator
    static std::optional<OperatorTraitInfo> getBinaryOperatorTrait(TokenType op);
    
    // Lookup the trait mapping for a given unary operator
    static std::optional<OperatorTraitInfo> getUnaryOperatorTrait(TokenType op);
    
    // Lookup the trait mapping for index access (either Index or IndexMut)
    static OperatorTraitInfo getIndexOperatorTrait(bool isMutable);
};

} // namespace fl
