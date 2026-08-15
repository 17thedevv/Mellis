#include "mellis/MiddleEnd/Semantic/BorrowAnalyzer.h"

namespace fl {

Place BorrowAnalyzer::resolvePlace(const mvir::Operand& op, const BorrowStateData& state) const {
    if (auto* locId = mvir::getLocalIf(op)) {
        auto it = state.placeMap.find(locId->name);
        if (it != state.placeMap.end()) {
            return it->second;
        }
    }
    return Place(op);
}

void BorrowAnalyzer::issueLoan(const Place& place, bool isMut, const std::string& refId, SourceLocation loc, BorrowStateData& state) {
    for (const auto& loan : state.activeLoans) {
        if (loan.place.overlapsWith(place)) {
            if (loan.isMutable || isMut) {
                std::string msg = "Cannot borrow '" + place.toString() + "' as " + 
                                  (isMut ? "mutable" : "immutable") + 
                                  " because it is already borrowed as " + 
                                  (loan.isMutable ? "mutable" : "immutable");
                diag_.error(loc, msg);
                hasError_ = true;
            }
        }
    }
    
    Loan newLoan;
    newLoan.id = nextLoanId_++;
    newLoan.place = place;
    newLoan.isMutable = isMut;
    newLoan.referenceId = refId;
    newLoan.loc = loc;
    
    state.activeLoans.push_back(newLoan);
}

void BorrowAnalyzer::checkAccess(const Place& place, bool isMut, SourceLocation loc, const BorrowStateData& state) {
    for (const auto& loan : state.activeLoans) {
        if (loan.place.overlapsWith(place)) {
            if (isMut) {
                diag_.error(loc, "Cannot mutate or move out of '" + place.toString() + "' because it is currently borrowed");
                hasError_ = true;
            } else if (loan.isMutable) {
                diag_.error(loc, "Cannot read '" + place.toString() + "' because it is currently borrowed as mutable");
                hasError_ = true;
            }
        }
    }
}

void BorrowAnalyzer::initEntryState(const mvir::Function& func, BorrowStateData& state) {
    // Nothing to initialize for borrows at entry.
}

bool BorrowAnalyzer::merge(BorrowStateData& dest, const BorrowStateData& src) {
    bool changed = false;
    for (const auto& pa : src.placeMap) {
        if (dest.placeMap.find(pa.first) == dest.placeMap.end()) {
            dest.placeMap[pa.first] = pa.second;
            changed = true;
        }
    }
    
    // For loans, conservative approach: a loan is active if it is active in ANY predecessor.
    for (const auto& srcLoan : src.activeLoans) {
        bool found = false;
        for (const auto& destLoan : dest.activeLoans) {
            if (destLoan.place == srcLoan.place && destLoan.isMutable == srcLoan.isMutable) {
                found = true;
                break;
            }
        }
        if (!found) {
            dest.activeLoans.push_back(srcLoan);
            changed = true;
        }
    }
    
    return changed;
}

void BorrowAnalyzer::transferInstruction(const mvir::Instruction& inst, BorrowStateData& state) {
    SourceLocation fakeLoc{0, 0, 0}; 

    if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(&inst)) {
        Place basePlace = resolvePlace(borrow->base, state);
        issueLoan(basePlace, borrow->isMutable, borrow->dest.name, fakeLoc, state);
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(&inst)) {
        Place srcPlace = resolvePlace(load->ptr, state);
        bool isMove = load->type && !load->type->isCopy();
        checkAccess(srcPlace, isMove /* isMut */, fakeLoc, state);
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(&inst)) {
        Place destPlace = resolvePlace(store->ptr, state);
        checkAccess(destPlace, true /* isMut */, fakeLoc, state);
        
        if (auto* locId = mvir::getLocalIf(store->value)) {
            Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
            checkAccess(srcPlace, false /* isMut */, fakeLoc, state);
            
            // Transfer reference ownership if this is a reference
            std::string srcName = locId->name;
            for (auto& loan : state.activeLoans) {
                if (loan.referenceId == srcName) {
                    loan.referenceId = destPlace.toString();
                }
            }
        }
    }
    else if (auto* call = dynamic_cast<const mvir::CallInst*>(&inst)) {
        for (size_t i = 0; i < call->args.size(); ++i) {
            auto& arg = call->args[i];
            if (auto* locId = mvir::getLocalIf(arg)) {
                bool isMove = false;
                if (call->funcType && i < call->funcType->paramTypes.size()) {
                    isMove = !call->funcType->paramTypes[i]->isCopy();
                }
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), isMove, fakeLoc, state);
            }
        }
    }
    else if (auto* vcall = dynamic_cast<const mvir::VirtualCallInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(vcall->receiver)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
        for (size_t i = 0; i < vcall->args.size(); ++i) {
            auto& arg = vcall->args[i];
            if (auto* locId = mvir::getLocalIf(arg)) {
                bool isMove = false;
                if (vcall->methodType && i < vcall->methodType->paramTypes.size()) {
                    isMove = !vcall->methodType->paramTypes[i]->isCopy();
                }
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), isMove, fakeLoc, state);
            }
        }
    }
    else if (auto* ret = dynamic_cast<const mvir::CastInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ret->value)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
    }
    else if (auto* un = dynamic_cast<const mvir::UnaryInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(un->operand)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
    }
    else if (auto* alu = dynamic_cast<const mvir::AluInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(alu->left)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
        if (auto* locId = mvir::getLocalIf(alu->right)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
    }
    else if (auto* ext = dynamic_cast<const mvir::ExtractInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ext->base)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
    }
    else if (auto* tag = dynamic_cast<const mvir::TagInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(tag->base)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
    }
    else if (auto* var = dynamic_cast<const mvir::VariantInst*>(&inst)) {
        for (const auto& arg : var->args) {
            if (auto* locId = mvir::getLocalIf(arg)) {
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
            }
        }
    }

    // Process loan expiration based on Liveness!
    // Rust NLL: A loan expires when the reference variable is no longer live.
    for (auto it = state.activeLoans.begin(); it != state.activeLoans.end(); ) {
        if (!it->referenceId.empty() && liveness_.liveInstructions[it->referenceId].count(&inst) == 0) {
            it = state.activeLoans.erase(it);
        } else {
            ++it;
        }
    }
}

void BorrowAnalyzer::transferTerminator(const mvir::Terminator& term, BorrowStateData& state) {
    SourceLocation fakeLoc{0, 0, 0}; 
    if (auto* br = dynamic_cast<const mvir::BranchTerm*>(&term)) {
        if (auto* locId = mvir::getLocalIf(br->condition)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
    } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(&term)) {
        if (auto* locId = mvir::getLocalIf(sw->condition)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
        }
    } else if (auto* ret = dynamic_cast<const mvir::RetTerm*>(&term)) {
        if (ret->value) {
            if (auto* locId = mvir::getLocalIf((*ret->value))) {
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), false, fakeLoc, state);
            }
        }
    }
    
    // Expire loans at end of block
    auto bIt = termToBlock_.find(&term);
    if (bIt != termToBlock_.end()) {
        const mvir::BasicBlock* block = bIt->second;
        const auto& outSet = liveness_.blockLiveOut[block];
        for (auto it = state.activeLoans.begin(); it != state.activeLoans.end(); ) {
            if (!it->referenceId.empty() && outSet.count(it->referenceId) == 0) {
                it = state.activeLoans.erase(it);
            } else {
                ++it;
            }
        }
    }
}

} // namespace fl
