#pragma once

#include "mellis/IR/MVIR.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/MiddleEnd/Semantic/InitializationAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/MoveAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/BorrowAnalyzer.h"
#include "mellis/MiddleEnd/Place.h"
#include "mellis/MiddleEnd/TraitSolver.h"
#include "mellis/MiddleEnd/SymbolTable.h"

namespace fl {

class SemanticAnalyzer {
    const mvir::Module* module_;
    DiagnosticEngine& diag_;
    SymbolTable& symTable_;
    std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap_;
    TraitSolver* solver_;

public:
    SemanticAnalyzer(const mvir::Module* module, DiagnosticEngine& diag, SymbolTable& symTable,
                     std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap, TraitSolver* solver = nullptr)
        : module_(module), diag_(diag), symTable_(symTable), closureStorageMap_(closureStorageMap), solver_(solver) {}

    bool analyze();
};

} // namespace fl
