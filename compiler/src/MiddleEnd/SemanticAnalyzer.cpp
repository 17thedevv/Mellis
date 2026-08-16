#include "mellis/MiddleEnd/Semantic/SemanticAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/DropElaborator.h"
#include <iostream>

namespace fl {

bool SemanticAnalyzer::analyze() {
    bool overallSuccess = true;
    for (const auto& func : module_->functions) {
        if (func->blocks.empty()) continue;

        InitializationAnalyzer initAnalyzer(module_, diag_);
        if (!initAnalyzer.analyzeFunction(*func)) {
            overallSuccess = false;
            // Stop early for this function to prevent cascading errors
            continue; 
        }

        MoveAnalyzer moveAnalyzer(module_, diag_, closureStorageMap_);
        if (!moveAnalyzer.analyzeFunction(*func)) {
            overallSuccess = false;
            // Stop early
            continue;
        }

        BorrowAnalyzer borrowAnalyzer(module_, diag_, symTable_, closureStorageMap_);
        if (!borrowAnalyzer.analyzeFunction(*func)) {
            overallSuccess = false;
            continue;
        }
    }

    if (overallSuccess) {
        DropElaborator dropElab(const_cast<mvir::Module*>(module_), closureStorageMap_);
        dropElab.run();
    }
    return overallSuccess;
}

} // namespace fl
