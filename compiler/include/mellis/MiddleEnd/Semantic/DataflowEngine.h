#pragma once

#include <vector>
#include <unordered_map>
#include <unordered_set>
#include "mellis/IR/MVIR.h"

namespace fl {

template <typename StateT>
class DataflowPass {
public:
    std::unordered_map<const mvir::BasicBlock*, StateT> inState;
    std::unordered_map<const mvir::BasicBlock*, StateT> outState;
    virtual ~DataflowPass() = default;

    virtual void transferInstruction(const mvir::Instruction& inst, StateT& state) = 0;
    virtual void transferTerminator(const mvir::Terminator& term, StateT& state) = 0;
    
    // Merge src into dest. Return true if dest was changed.
    virtual bool merge(StateT& dest, const StateT& src) = 0;

    virtual void initEntryState(const mvir::Function& func, StateT& state) = 0;

    bool run(const mvir::Function& func) {
        if (func.blocks.empty()) return true;
        inState.clear();
        outState.clear();

        inState.clear();
        outState.clear();
        
        std::unordered_map<const mvir::BasicBlock*, std::vector<const mvir::BasicBlock*>> preds;
        
        std::vector<const mvir::BasicBlock*> worklist;
        std::unordered_set<const mvir::BasicBlock*> inWorklist;

        auto findBlock = [&](const mvir::Function& f, const mvir::LabelId& target) -> const mvir::BasicBlock* {
            for (const auto& block : f.blocks) {
                if (block->label.name == target.name) {
                    return block.get();
                }
            }
            return nullptr;
        };

        for (const auto& b : func.blocks) {
            if (auto* br = dynamic_cast<const mvir::BranchTerm*>(b->terminator.get())) {
                if (auto* tTarget = findBlock(func, br->trueTarget)) preds[tTarget].push_back(b.get());
                if (auto* fTarget = findBlock(func, br->falseTarget)) preds[fTarget].push_back(b.get());
            } else if (auto* jmp = dynamic_cast<const mvir::JumpTerm*>(b->terminator.get())) {
                if (auto* tTarget = findBlock(func, jmp->target)) preds[tTarget].push_back(b.get());
            } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(b->terminator.get())) {
                if (auto* dTarget = findBlock(func, sw->defaultTarget)) preds[dTarget].push_back(b.get());
                for(const auto& c : sw->cases) {
                    if (auto* cTarget = findBlock(func, c.second)) preds[cTarget].push_back(b.get());
                }
            }
        }

        worklist.push_back(func.blocks.front().get());
        inWorklist.insert(func.blocks.front().get());
        initEntryState(func, outState[func.blocks.front().get()]);

        while (!worklist.empty()) {
            const mvir::BasicBlock* block = worklist.front();
            worklist.erase(worklist.begin());
            inWorklist.erase(block);

            StateT state;
            auto pit = preds.find(block);
            if (pit != preds.end() && !pit->second.empty()) {
                state = outState[pit->second[0]];
                for (size_t i = 1; i < pit->second.size(); ++i) {
                    merge(state, outState[pit->second[i]]);
                }
            } else if (block == func.blocks.front().get()) {
                initEntryState(func, state);
            }

            inState[block] = state;

            for (const auto& inst : block->instructions) {
                transferInstruction(*inst, state);
            }
            if (block->terminator) {
                transferTerminator(*block->terminator, state);
            }

            if (outState.find(block) == outState.end() || !(outState[block] == state)) {
                outState[block] = state;
                
                auto addSucc = [&](const mvir::LabelId& target) {
                    if (auto* targetBlock = findBlock(func, target)) {
                        if (inWorklist.insert(targetBlock).second) {
                            worklist.push_back(targetBlock);
                        }
                    }
                };

                if (auto* br = dynamic_cast<const mvir::BranchTerm*>(block->terminator.get())) {
                    addSucc(br->trueTarget);
                    addSucc(br->falseTarget);
                } else if (auto* jmp = dynamic_cast<const mvir::JumpTerm*>(block->terminator.get())) {
                    addSucc(jmp->target);
                } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(block->terminator.get())) {
                    addSucc(sw->defaultTarget);
                    for (const auto& c : sw->cases) addSucc(c.second);
                }
            }
        }

        return true; 
    }
};

} // namespace fl
