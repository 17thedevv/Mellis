#include "mellis/MiddleEnd/LivenessAnalyzer.h"
#include <algorithm>

namespace fl {

// Helper method to find block by label
static const mvir::BasicBlock* findBlockLA(const mvir::Function& func, const mvir::LabelId& label) {
    for (const auto& block : func.blocks) {
        if (block->label == label) return block.get();
    }
    return nullptr;
}

LivenessInfo LivenessAnalyzer::computeLiveness(const mvir::Function& func) {
    LivenessInfo liveness;
    std::unordered_map<const mvir::BasicBlock*, std::unordered_set<std::string>> liveIn;
    std::unordered_map<const mvir::BasicBlock*, std::unordered_set<std::string>> liveOut;

    bool changed = true;
    while (changed) {
        changed = false;
        for (auto it = func.blocks.rbegin(); it != func.blocks.rend(); ++it) {
            const mvir::BasicBlock* block = it->get();
            
            std::unordered_set<std::string> oldIn = liveIn[block];
            std::unordered_set<std::string> out;
            
            if (auto* br = dynamic_cast<const mvir::BranchTerm*>(block->terminator.get())) {
                if (auto* tTarget = findBlockLA(func, br->trueTarget)) {
                    for (const auto& v : liveIn[tTarget]) out.insert(v);
                }
                if (auto* fTarget = findBlockLA(func, br->falseTarget)) {
                    for (const auto& v : liveIn[fTarget]) out.insert(v);
                }
            } else if (auto* jmp = dynamic_cast<const mvir::JumpTerm*>(block->terminator.get())) {
                if (auto* tTarget = findBlockLA(func, jmp->target)) {
                    for (const auto& v : liveIn[tTarget]) out.insert(v);
                }
            } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(block->terminator.get())) {
                if (auto* dTarget = findBlockLA(func, sw->defaultTarget)) {
                    for (const auto& v : liveIn[dTarget]) out.insert(v);
                }
                for (const auto& c : sw->cases) {
                    if (auto* cTarget = findBlockLA(func, c.second)) {
                        for (const auto& v : liveIn[cTarget]) out.insert(v);
                    }
                }
            }
            
            liveOut[block] = out;
            std::unordered_set<std::string> in = out;
            
            if (auto* br = dynamic_cast<const mvir::BranchTerm*>(block->terminator.get())) {
                if (auto* loc = mvir::getLocalIf(br->condition)) {
                    in.insert(loc->name);
                }
            } else if (auto* ret = dynamic_cast<const mvir::RetTerm*>(block->terminator.get())) {
                if (ret->value) {
                    if (auto* loc = mvir::getLocalIf(*ret->value)) {
                        in.insert(loc->name);
                    }
                }
            } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(block->terminator.get())) {
                if (auto* loc = mvir::getLocalIf(sw->condition)) {
                    in.insert(loc->name);
                }
            }
            
            for (auto instIt = block->instructions.rbegin(); instIt != block->instructions.rend(); ++instIt) {
                const mvir::Instruction* inst = instIt->get();
                
                // Lu LiveOut c?a l?nh ny
                for (const auto& var : in) {
                    liveness.liveInstructions[var].insert(inst); 
                }
                
                std::string destName = "";
                if (auto* local = dynamic_cast<const mvir::LocalInst*>(inst)) destName = local->dest.name;
                else if (auto* load = dynamic_cast<const mvir::LoadInst*>(inst)) destName = load->dest.name;
                else if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(inst)) destName = borrow->dest.name;
                else if (auto* alu = dynamic_cast<const mvir::AluInst*>(inst)) destName = alu->dest.name;
                else if (auto* un = dynamic_cast<const mvir::UnaryInst*>(inst)) destName = un->dest.name;
                else if (auto* call = dynamic_cast<const mvir::CallInst*>(inst)) {
                    if (call->dest) destName = call->dest->name;
                }

                else if (auto* cast = dynamic_cast<const mvir::CastInst*>(inst)) destName = cast->dest.name;
                else if (auto* ext = dynamic_cast<const mvir::ExtractInst*>(inst)) destName = ext->dest.name;
                else if (auto* tag = dynamic_cast<const mvir::TagInst*>(inst)) destName = tag->dest.name;
                else if (auto* varInst = dynamic_cast<const mvir::VariantInst*>(inst)) destName = varInst->dest.name;
                else if (auto* virtCall = dynamic_cast<const mvir::VirtualCallInst*>(inst)) {
                    if (virtCall->dest) destName = virtCall->dest->name;
                }
                else if (auto* mkTrait = dynamic_cast<const mvir::MakeTraitObjectInst*>(inst)) destName = mkTrait->dest.name;
                else if (auto* awt = dynamic_cast<const mvir::AwaitInst*>(inst)) destName = awt->dest.name;
                else if (auto* size = dynamic_cast<const mvir::SizeofInst*>(inst)) destName = size->dest.name;
                else if (auto* align = dynamic_cast<const mvir::AlignofInst*>(inst)) destName = align->dest.name;
                else if (auto* icall = dynamic_cast<const mvir::IntrinsicCallInst*>(inst)) {
                    if (icall->dest) destName = icall->dest->name;
                }

                if (!destName.empty()) {
                    in.erase(destName);
                }
                
                auto addUse = [&](const mvir::Operand& op) {
                    if (auto* loc = mvir::getLocalIf(op)) {
                        in.insert(loc->name);
                    }
                };
                
                if (auto* load = dynamic_cast<const mvir::LoadInst*>(inst)) addUse(load->ptr);
                else if (auto* store = dynamic_cast<const mvir::StoreInst*>(inst)) { addUse(store->ptr); addUse(store->value); }
                else if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(inst)) addUse(borrow->base);
                else if (auto* alu = dynamic_cast<const mvir::AluInst*>(inst)) { addUse(alu->left); addUse(alu->right); }
                else if (auto* un = dynamic_cast<const mvir::UnaryInst*>(inst)) { addUse(un->operand); }
                else if (auto* call = dynamic_cast<const mvir::CallInst*>(inst)) {
                    addUse(call->func);
                    for (auto& arg : call->args) addUse(arg);
                }
                else if (auto* cast = dynamic_cast<const mvir::CastInst*>(inst)) { addUse(cast->value); }
                else if (auto* ext = dynamic_cast<const mvir::ExtractInst*>(inst)) { addUse(ext->base); }
                else if (auto* tag = dynamic_cast<const mvir::TagInst*>(inst)) { addUse(tag->base); }
                else if (auto* varInst = dynamic_cast<const mvir::VariantInst*>(inst)) { 
                    for(auto& arg : varInst->args) addUse(arg); 
                }
                else if (auto* virtCall = dynamic_cast<const mvir::VirtualCallInst*>(inst)) {
                    addUse(virtCall->receiver);
                    for (auto& arg : virtCall->args) addUse(arg);
                }
                else if (auto* mkTrait = dynamic_cast<const mvir::MakeTraitObjectInst*>(inst)) {
                    addUse(mkTrait->value);
                }
                else if (auto* awt = dynamic_cast<const mvir::AwaitInst*>(inst)) {
                    addUse(awt->futureVal);
                }
                else if (auto* icall = dynamic_cast<const mvir::IntrinsicCallInst*>(inst)) {
                    for (auto& arg : icall->args) addUse(arg);
                }
            }
            
            if (in != oldIn) {
                liveIn[block] = in;
                changed = true;
            }
        }
    }
    
    liveness.blockLiveOut = liveOut;
    return liveness;
}

} // namespace fl
