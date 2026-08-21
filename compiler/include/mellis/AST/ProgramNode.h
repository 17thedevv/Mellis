#pragma once
#include "mellis/AST/ASTNode.h"
#include <vector>
#include <memory>

namespace fl {

class ItemNode;

class ProgramNode : public ASTNode {
public:
    std::vector<std::unique_ptr<ItemNode>> items;
    void accept(ASTVisitor& v) override;
};

} // namespace fl
