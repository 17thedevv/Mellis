#include "mellis/MiddleEnd/Semantic/DropElaborator.h"
#include "mellis/Support/Diagnostic.h"
#include <iostream>

namespace fl {

void DropElaborator::run() {
    for (auto& func : module_->functions) {
        if (!func->blocks.empty()) {
            elaborateFunction(*func);
        }
    }
}

void DropElaborator::elaborateFunction(mvir::Function& func) {
    DiagnosticEngine dummyDiag; // Ignore errors, analyzers already run elsewhere
    
    InitializationAnalyzer initAnalyzer(module_, dummyDiag);
    initAnalyzer.analyzeFunction(func);
    
    MoveAnalyzer moveAnalyzer(module_, dummyDiag);
    moveAnalyzer.analyzeFunction(func);
    
    for (auto& block : func.blocks) {
        auto initState = initAnalyzer.inState[block.get()];
        auto moveState = moveAnalyzer.inState[block.get()];
        
        for (auto it = block->instructions.begin(); it != block->instructions.end(); ) {
            const mvir::Instruction* inst = it->get();
            
            if (auto* drop = dynamic_cast<const mvir::DropInst*>(inst)) {
                // Determine state of the dropped place
                Place place(drop->value);
                
                bool shouldDrop = true;
                
                // Check if uninitialized
                auto initIt = initState.initStateMap.find(place.toString());
                if (initIt == initState.initStateMap.end() || initIt->second == InitState::Uninitialized) {
                    shouldDrop = false;
                }
                
                // Check if moved
                auto moveIt = moveState.stateMap.find(place.toString());
                if (moveIt != moveState.stateMap.end() && moveIt->second != MoveState::Valid) {
                    shouldDrop = false;
                }
                
                if (!shouldDrop) {
                    it = block->instructions.erase(it);
                    continue; 
                }
            }
            
            initAnalyzer.transferInstruction(*inst, initState);
            moveAnalyzer.transferInstruction(*inst, moveState);
            ++it;
        }
    }
}

} // namespace fl
