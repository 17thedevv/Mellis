#pragma once

#include "mellis/IR/MVIR.h"
#include "mellis/MiddleEnd/Semantic/MoveAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/InitializationAnalyzer.h"

namespace fl {

class DropElaborator {
    mvir::Module* module_;
    std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap_;

public:
    explicit DropElaborator(mvir::Module* module, std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap) 
        : module_(module), closureStorageMap_(closureStorageMap) {}

    void run();

private:
    void elaborateFunction(mvir::Function& func);
};

} // namespace fl
