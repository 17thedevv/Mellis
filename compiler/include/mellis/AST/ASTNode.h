#pragma once
#include <cstdint>
#include <stdexcept>
#include <memory>

#include "mellis/Core/SourceLocation.h"

namespace fl {

using ExpansionID = uint32_t;

class ASTVisitor;

class ASTNode {
public:
    SourceLocation loc;
    SourceLocation endLoc;
    ExpansionID expansionID = 0;
    virtual ~ASTNode() = default;
    virtual void accept(ASTVisitor& v) = 0;

    virtual ASTNode* cloneImpl() const {
        throw std::runtime_error("cloneImpl not implemented");
    }

    std::unique_ptr<ASTNode> clone() const {
        return std::unique_ptr<ASTNode>(cloneImpl());
    }

    template <typename T>
    std::unique_ptr<T> cloneAs() const {
        return std::unique_ptr<T>(dynamic_cast<T*>(cloneImpl()));
    }
};

class ItemNode : public ASTNode {};

} // namespace fl