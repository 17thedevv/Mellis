#pragma once

#include "mellis/IR/MVIR.h"
#include "mellis/MiddleEnd/Semantic/MoveAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/InitializationAnalyzer.h"

namespace fl {

class DropElaborator {
    mvir::Module* module_;
    std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap_;
    TraitSolver* solver_;
    SymbolTable* symTable_;

public:
    explicit DropElaborator(mvir::Module* module, std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap, TraitSolver* solver = nullptr, SymbolTable* symTable = nullptr) 
        : module_(module), closureStorageMap_(closureStorageMap), solver_(solver), symTable_(symTable) {}

    void run();

private:
    void elaborateFunction(mvir::Function& func);
};

} // namespace fl
