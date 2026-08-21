#include "mellis/MiddleEnd/Semantic/BorrowAnalyzer.h"

namespace fl {

std::vector<Place> BorrowAnalyzer::resolvePlace(const mvir::Operand& op, const BorrowStateData& state) const {
    if (auto* locId = mvir::getLocalIf(op)) {
        auto it = state.placeMap.find(locId->name);
        if (it != state.placeMap.end()) {
            return it->second;
        }
    }
    return { Place(op) };
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
        auto& destPlaces = dest.placeMap[pa.first];
        for (const auto& p : pa.second) {
            bool found = false;
            for (const auto& dp : destPlaces) {
                if (dp == p) { found = true; break; }
            }
            if (!found) {
                destPlaces.push_back(p);
                changed = true;
            }
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
        std::vector<Place> basePlaces = resolvePlace(borrow->base, state);
        for (const auto& basePlace : basePlaces) {
            issueLoan(basePlace, borrow->isMutable, borrow->dest.name, fakeLoc, state);
        }
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(&inst)) {
        std::vector<Place> srcPlaces = resolvePlace(load->ptr, state);
        bool isMove = load->type && !isCopy(load->type);
        for (const auto& srcPlace : srcPlaces) {
            checkAccess(srcPlace, isMove /* isMut */, fakeLoc, state);
        }
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(&inst)) {
        std::vector<Place> destPlaces = resolvePlace(store->ptr, state);
        for (const auto& destPlace : destPlaces) {
            checkAccess(destPlace, true /* isMut */, fakeLoc, state);
        }
        
        if (auto* locId = mvir::getLocalIf(store->value)) {
            std::vector<Place> srcPlaces = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
            for (const auto& srcPlace : srcPlaces) {
                checkAccess(srcPlace, false /* isMut */, fakeLoc, state);
            }
            
            // Transfer reference ownership if this is a reference
            std::string srcName = locId->name;
            for (auto& loan : state.activeLoans) {
                if (loan.referenceId == srcName) {
                    // Update referenceId to all possible destPlaces' string representations
                    if (!destPlaces.empty()) {
                        loan.referenceId = destPlaces[0].toString(); // Simplification: we might lose tracking if multiple dest places
                    }
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
                    isMove = !isCopy(call->funcType->paramTypes[i]);
                }
                for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) {
                    checkAccess(p, isMove, fakeLoc, state);
                }
            }
        }
    }
    else if (auto* vcall = dynamic_cast<const mvir::VirtualCallInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(vcall->receiver)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
        for (size_t i = 0; i < vcall->args.size(); ++i) {
            auto& arg = vcall->args[i];
            if (auto* locId = mvir::getLocalIf(arg)) {
                bool isMove = false;
                if (vcall->methodType && i < vcall->methodType->paramTypes.size()) {
                    isMove = !isCopy(vcall->methodType->paramTypes[i]);
                }
                for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, isMove, fakeLoc, state);
            }
        }
    }
    else if (auto* ret = dynamic_cast<const mvir::CastInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ret->value)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
    }
    else if (auto* un = dynamic_cast<const mvir::UnaryInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(un->operand)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
    }
    else if (auto* alu = dynamic_cast<const mvir::AluInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(alu->left)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
        if (auto* locId = mvir::getLocalIf(alu->right)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
    }
    else if (auto* ext = dynamic_cast<const mvir::ExtractInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ext->base)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
    }
    else if (auto* tag = dynamic_cast<const mvir::TagInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(tag->base)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
    }
    else if (auto* var = dynamic_cast<const mvir::VariantInst*>(&inst)) {
        for (const auto& arg : var->args) {
            if (auto* locId = mvir::getLocalIf(arg)) {
                for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
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
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
    } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(&term)) {
        if (auto* locId = mvir::getLocalIf(sw->condition)) {
            for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
        }
    } else if (auto* ret = dynamic_cast<const mvir::RetTerm*>(&term)) {
        if (ret->value) {
            if (auto* locId = mvir::getLocalIf((*ret->value))) {
                for (const auto& p : resolvePlace(mvir::Operand(mvir::Place(*locId)), state)) checkAccess(p, false, fakeLoc, state);
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

bool BorrowAnalyzer::isCopy(const Type* t) const {
    if (!t) return false;
    if (auto* closureTy = dynamic_cast<const ClosureType*>(t)) {
        if (closureStorageMap_.find(closureTy) != closureStorageMap_.end() && 
            closureStorageMap_.at(closureTy) == ClosureStorageKind::Heap) {
            return false; // Heap closures are Move-Only
        }
    }

    if (t->isCopy()) return true;

    if (solver_) {
        std::optional<SymbolID> copyTraitId = symTable_.getWellKnownTrait(WellKnownTrait::Copy);
        if (copyTraitId.has_value()) {
            Goal goal;
            goal.kind = GoalKind::Trait;
            goal.selfType = t;
            goal.traitId = *copyTraitId;
            return solver_->solve(goal).result == SolverResult::Success;
        }
    }
    return false;
}

} // namespace fl
