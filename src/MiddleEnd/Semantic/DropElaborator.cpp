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
    
    MoveAnalyzer moveAnalyzer(module_, dummyDiag, closureStorageMap_);
    moveAnalyzer.analyzeFunction(func);
    
    for (auto& block : func.blocks) {
        auto initState = initAnalyzer.inState[block.get()];
        auto moveState = moveAnalyzer.inState[block.get()];
        
        for (auto it = block->instructions.begin(); it != block->instructions.end(); ) {
            const mvir::Instruction* inst = it->get();
            
            bool isHeapClosureDrop = false;
            mvir::Operand dropOperand;

            if (auto* drop = dynamic_cast<const mvir::DropInst*>(inst)) {
                // Determine state of the dropped place
                Place place(drop->value);
                
                bool shouldDrop = true;
                
                // Check if moved
                auto moveIt = moveState.stateMap.find(place.toString());
                if (moveIt != moveState.stateMap.end() && moveIt->second != MoveState::Valid) {
                    std::cerr << "[DEBUG DropElaborator] " << place.toString() << " is MOVED! Removing drop." << std::endl;
                    shouldDrop = false;
                }

                // Check if initialized
                auto initIt = initState.initStateMap.find(place.toString());
                if (initIt == initState.initStateMap.end() || initIt->second != InitState::Initialized) {
                    std::cerr << "[DEBUG DropElaborator] " << place.toString() << " is UNINITIALIZED! Removing drop. initIt exists? " << (initIt != initState.initStateMap.end()) << std::endl;
                    shouldDrop = false;
                }
                
                if (!shouldDrop) {
                    it = block->instructions.erase(it);
                    continue; 
                }

                if (auto* ptrTy = dynamic_cast<const PointerType*>(drop->type)) {
                    if (auto* cTy = dynamic_cast<const ClosureType*>(ptrTy->pointee)) {
                        auto it = closureStorageMap_.find(cTy);
                        if (it != closureStorageMap_.end() && it->second == ClosureStorageKind::Heap) {
                            isHeapClosureDrop = true;
                            dropOperand = drop->value;
                        }
                    }
                }
            }
            
            initAnalyzer.transferInstruction(*inst, initState);
            moveAnalyzer.transferInstruction(*inst, moveState);
            ++it;

            if (isHeapClosureDrop) {
                auto freeInst = std::make_unique<mvir::HeapFreeInst>(dropOperand);
                it = block->instructions.insert(it, std::move(freeInst));
                ++it;
            }
        }
    }
}

} // namespace fl
