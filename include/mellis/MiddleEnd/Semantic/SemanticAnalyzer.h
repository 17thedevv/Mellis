#pragma once

#include "mellis/IR/MVIR.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/MiddleEnd/Semantic/InitializationAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/MoveAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/BorrowAnalyzer.h"
#include "mellis/MiddleEnd/SymbolTable.h"

namespace fl {

class SemanticAnalyzer {
    const mvir::Module* module_;
    DiagnosticEngine& diag_;
    SymbolTable& symTable_;

public:
    SemanticAnalyzer(const mvir::Module* module, DiagnosticEngine& diag, SymbolTable& symTable)
        : module_(module), diag_(diag), symTable_(symTable) {}

    bool analyze();
};

} // namespace fl
