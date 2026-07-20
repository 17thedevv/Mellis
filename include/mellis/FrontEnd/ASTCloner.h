#pragma once
#include "mellis/AST/ASTNode.h"
#include <memory>

namespace fl {

class ASTCloner {
public:
    template <typename T>
    std::unique_ptr<T> clone(const T& node) {
        return node.template cloneAs<T>();
    }
};

} // namespace fl
