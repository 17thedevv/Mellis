#pragma once
#include <cstdint>

#include "fdlang/Core/SourceLocation.h"

namespace fl {

class ASTVisitor;

class ASTNode {
public:
    SourceLocation loc;
    virtual ~ASTNode() = default;
    virtual void accept(ASTVisitor& v) = 0;
};

class ItemNode : public ASTNode {};

} // namespace fl