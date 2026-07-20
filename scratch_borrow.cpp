#include "mellis/MiddleEnd/BorrowChecker.h"
#include <iostream>
#include <algorithm>

namespace fl {

// Helper method to find block by label
const mvir::BasicBlock* findBlock(const mvir::Function& func, const mvir::LabelId& label) {
    for (const auto& block : func.blocks) {
        if (block->label == label) return block.get();
    }
    return nullptr;
}

bool BorrowChecker::check() {
    bool ok = true;
    for (const auto& func : module_->functions) {
        nextLoanId_ = 0;
        checkFunction(*func);
        if (diag_.hasErrors()) ok = false;
    }
    return ok;
}

// ============================================================================
// Pass 1: Liveness Analysis (Backward Dataflow)
// ============================================================================
LivenessInfo BorrowChecker::computeLiveness(const mvir::Function& func) {
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
                if (auto* tTarget = findBlock(func, br->trueTarget)) {
                    for (const auto& v : liveIn[tTarget]) out.insert(v);
                }
                if (auto* fTarget = findBlock(func, br->falseTarget)) {
                    for (const auto& v : liveIn[fTarget]) out.insert(v);
                }
            } else if (auto* jmp = dynamic_cast<const mvir::JumpTerm*>(block->terminator.get())) {
                if (auto* tTarget = findBlock(func, jmp->target)) {
                    for (const auto& v : liveIn[tTarget]) out.insert(v);
                }
            } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(block->terminator.get())) {
                if (auto* dTarget = findBlock(func, sw->defaultTarget)) {
                    for (const auto& v : liveIn[dTarget]) out.insert(v);
                }
                for (const auto& c : sw->cases) {
                    if (auto* cTarget = findBlock(func, c.second)) {
                        for (const auto& v : liveIn[cTarget]) out.insert(v);
                    }
                }
            }
            
            liveOut[block] = out;
            std::unordered_set<std::string> in = out;
            
            if (auto* br = dynamic_cast<const mvir::BranchTerm*>(block->terminator.get())) {
                if (auto* loc = std::get_if<mvir::LocalId>(&br->condition)) {
                    in.insert(loc->name);
                }
            } else if (auto* ret = dynamic_cast<const mvir::RetTerm*>(block->terminator.get())) {
                if (ret->value) {
                    if (auto* loc = std::get_if<mvir::LocalId>(&*ret->value)) {
                        in.insert(loc->name);
                    }
                }
            } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(block->terminator.get())) {
                if (auto* loc = std::get_if<mvir::LocalId>(&sw->condition)) {
                    in.insert(loc->name);
                }
            }
            
            for (auto instIt = block->instructions.rbegin(); instIt != block->instructions.rend(); ++instIt) {
                const mvir::Instruction* inst = instIt->get();
                
                // Lýu LiveOut c?a l?nh này
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
                else if (auto* idx = dynamic_cast<const mvir::IndexInst*>(inst)) destName = idx->dest.name;
                else if (auto* fld = dynamic_cast<const mvir::FieldInst*>(inst)) destName = fld->dest.name;
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
                
                if (!destName.empty()) {
                    in.erase(destName);
                }
                
                auto addUse = [&](const mvir::Operand& op) {
                    if (auto* loc = std::get_if<mvir::LocalId>(&op)) {
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
                else if (auto* idx = dynamic_cast<const mvir::IndexInst*>(inst)) { addUse(idx->base); addUse(idx->index); }
                else if (auto* fld = dynamic_cast<const mvir::FieldInst*>(inst)) { addUse(fld->base); }
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
            }
            
            if (in != oldIn) {
                liveIn[block] = in;
                changed = true;
            }
        }
    }
    
    return liveness;
}

// ============================================================================
// Pass 2: NLL Borrow Checking (Forward Dataflow)
// ============================================================================
void BorrowChecker::checkFunction(const mvir::Function& func) {
    if (func.blocks.empty()) return;
    LivenessInfo liveness = computeLiveness(func);
    DataflowState initialState;
    std::unordered_set<const mvir::BasicBlock*> visited;
    checkDataflow(func, *func.blocks.front(), initialState, liveness, visited);
}

void BorrowChecker::checkDataflow(const mvir::Function& func, const mvir::BasicBlock& block, DataflowState state, const LivenessInfo& liveness, std::unordered_set<const mvir::BasicBlock*>& visited) {
    if (visited.count(&block)) return;
    visited.insert(&block);

    for (const auto& instPtr : block.instructions) {
        const mvir::Instruction& inst = *instPtr;
        checkInstruction(inst, state, liveness);
        
        state.activeLoans.erase(
            std::remove_if(state.activeLoans.begin(), state.activeLoans.end(),
                [&](const Loan& loan) {
                    auto it = liveness.liveInstructions.find(loan.referenceId);
                    if (it != liveness.liveInstructions.end()) {
                        return it->second.find(&inst) == it->second.end();
                    }
                    return true; 
                }),
            state.activeLoans.end()
        );
    }
    
    // NLL Kill sau terminator
    state.activeLoans.erase(
        std::remove_if(state.activeLoans.begin(), state.activeLoans.end(),
            [&](const Loan& loan) { return false; }),
        state.activeLoans.end()
    );

    if (auto* br = dynamic_cast<const mvir::BranchTerm*>(block.terminator.get())) {
        if (auto* tTarget = findBlock(func, br->trueTarget)) checkDataflow(func, *tTarget, state, liveness, visited);
        if (auto* fTarget = findBlock(func, br->falseTarget)) checkDataflow(func, *fTarget, state, liveness, visited);
    } else if (auto* jmp = dynamic_cast<const mvir::JumpTerm*>(block.terminator.get())) {
        if (auto* tTarget = findBlock(func, jmp->target)) checkDataflow(func, *tTarget, state, liveness, visited);
    } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(block.terminator.get())) {
        if (auto* dTarget = findBlock(func, sw->defaultTarget)) checkDataflow(func, *dTarget, state, liveness, visited);
        for(const auto& c : sw->cases) {
            if (auto* cTarget = findBlock(func, c.second)) checkDataflow(func, *cTarget, state, liveness, visited);
        }
    }
}

Place BorrowChecker::resolvePlace(const mvir::Operand& op, const DataflowState& state) const {
    if (auto* locId = std::get_if<mvir::LocalId>(&op)) {
        auto it = state.placeMap.find(locId->name);
        if (it != state.placeMap.end()) {
            return it->second;
        }
    }
    return Place(op);
}

void BorrowChecker::checkInstruction(const mvir::Instruction& inst, DataflowState& state, const LivenessInfo& liveness) {
    SourceLocation fakeLoc{0, 0, 0}; 

    if (auto* idxInst = dynamic_cast<const mvir::IndexInst*>(&inst)) {
        Place basePlace = resolvePlace(idxInst->base, state);
        Place derefPlace = basePlace;
        derefPlace.projections.push_back(Projection{ProjectionKind::Deref, ""});
        state.placeMap[idxInst->dest.name] = derefPlace;
    }
    else if (auto* fieldInst = dynamic_cast<const mvir::FieldInst*>(&inst)) {
        Place basePlace = resolvePlace(fieldInst->base, state);
        Place fieldPlace = basePlace;
        fieldPlace.projections.push_back(Projection{ProjectionKind::Field, std::to_string(fieldInst->index)});
        state.placeMap[fieldInst->dest.name] = fieldPlace;
    }
    else if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(&inst)) {
        Place basePlace = resolvePlace(borrow->base, state);
        issueLoan(basePlace, borrow->isMutable, borrow->dest.name, fakeLoc, state);
        Place derefPlace = basePlace;
        derefPlace.projections.push_back(Projection{ProjectionKind::Deref, ""});
        state.placeMap[borrow->dest.name] = derefPlace;
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(&inst)) {
        Place srcPlace = resolvePlace(load->ptr, state);
        checkAccess(srcPlace, false /* isMut */, fakeLoc, state);
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(&inst)) {
        Place destPlace = resolvePlace(store->ptr, state);
        checkAccess(destPlace, true /* isMut */, fakeLoc, state);
    }
}

void BorrowChecker::issueLoan(const Place& place, bool isMut, const std::string& refId, SourceLocation loc, DataflowState& state) {
    for (const auto& loan : state.activeLoans) {
        if (loan.place.overlapsWith(place)) {
            if (loan.isMutable || isMut) {
                std::string msg = "Cannot borrow '" + place.toString() + "' as " + 
                                  (isMut ? "mutable" : "immutable") + 
                                  " because it is already borrowed as " + 
                                  (loan.isMutable ? "mutable" : "immutable");
                diag_.error(loc, msg);
                return;
            }
        }
    }
    state.activeLoans.push_back(Loan{nextLoanId_++, place, isMut, refId, loc});
}

void BorrowChecker::checkAccess(const Place& place, bool isMut, SourceLocation loc, const DataflowState& state) {
    for (const auto& loan : state.activeLoans) {
        if (loan.place.overlapsWith(place)) {
            if (isMut || loan.isMutable) {
                std::string msg = "Cannot " + std::string(isMut ? "mutate" : "access") + 
                                  " '" + place.toString() + "' because it is currently borrowed";
                diag_.error(loc, msg);
            }
        }
    }
}

} // namespace fl
