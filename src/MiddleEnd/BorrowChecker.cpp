#include "mellis/MiddleEnd/BorrowChecker.h"
#include <iostream>

namespace fl {

bool BorrowChecker::check() {
    bool ok = true;
    for (const auto& func : module_->functions) {
        // Reset state for each function
        placeMap_.clear();
        scopeStack_.clear();
        nextLoanId_ = 0;
        
        checkFunction(*func);
        if (diag_.hasErrors()) ok = false;
    }
    return ok;
}

void BorrowChecker::checkFunction(const mvir::Function& func) {
    scopeStack_.push_back({}); // global scope for function
    for (const auto& block : func.blocks) {
        checkBlock(*block);
    }
    scopeStack_.pop_back();
}

void BorrowChecker::checkBlock(const mvir::BasicBlock& block) {
    for (const auto& inst : block.instructions) {
        checkInstruction(*inst);
    }
}

Place BorrowChecker::resolvePlace(const mvir::Operand& op) const {
    if (auto* locId = std::get_if<mvir::LocalId>(&op)) {
        auto it = placeMap_.find(locId->name);
        if (it != placeMap_.end()) {
            return it->second;
        }
    }
    return Place(op);
}

void BorrowChecker::checkInstruction(const mvir::Instruction& inst) {
    // Basic intra-procedural dataflow for borrow checker.
    SourceLocation fakeLoc{0, 0, 0}; // TODO: Pass source location in MVIR if possible

    if (auto* idxInst = dynamic_cast<const mvir::IndexInst*>(&inst)) {
        Place basePlace = resolvePlace(idxInst->base);
        // Append projection
        Place derefPlace = basePlace;
        derefPlace.projections.push_back(Projection{ProjectionKind::Deref, ""});
        placeMap_[idxInst->dest.name] = derefPlace;
    }
    else if (auto* fieldInst = dynamic_cast<const mvir::FieldInst*>(&inst)) {
        Place basePlace = resolvePlace(fieldInst->base);
        Place fieldPlace = basePlace;
        fieldPlace.projections.push_back(Projection{ProjectionKind::Field, std::to_string(fieldInst->index)});
        placeMap_[fieldInst->dest.name] = fieldPlace;
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(&inst)) {
        Place srcPlace = resolvePlace(load->ptr);
        checkAccess(srcPlace, false /* isMut */, fakeLoc);
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(&inst)) {
        Place destPlace = resolvePlace(store->ptr);
        checkAccess(destPlace, true /* isMut */, fakeLoc);
    }
    else if (dynamic_cast<const mvir::BeginScopeInst*>(&inst)) {
        scopeStack_.push_back({});
    }
    else if (dynamic_cast<const mvir::EndScopeInst*>(&inst)) {
        if (!scopeStack_.empty()) {
            scopeStack_.pop_back();
        }
    }
    else if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(&inst)) {
        Place basePlace = resolvePlace(borrow->base);
        issueLoan(basePlace, borrow->isMutable, fakeLoc);
        // The destination register represents the borrowed reference.
        // For MVP, we can map it to a deref place so that if they use it, it maps back.
        // E.g. %2 = borrow mut %1  =>  placeMap_["%2"] points to `*Place(%1)`
        Place derefPlace = basePlace;
        derefPlace.projections.push_back(Projection{ProjectionKind::Deref, ""});
        placeMap_[borrow->dest.name] = derefPlace;
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(&inst)) {
        Place srcPlace = resolvePlace(load->ptr);
        checkAccess(srcPlace, false /* isMut */, fakeLoc);
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(&inst)) {
        Place destPlace = resolvePlace(store->ptr);
        checkAccess(destPlace, true /* isMut */, fakeLoc);
    }
}

void BorrowChecker::issueLoan(const Place& place, bool isMut, SourceLocation loc) {
    // 1. Check if we can issue this loan based on active loans.
    for (const auto& scope : scopeStack_) {
        for (const auto& loan : scope) {
        if (loan.place.overlapsWith(place)) {
            if (loan.isMutable || isMut) {
                // Conflict!
                std::string msg = "Cannot borrow '" + place.toString() + "' as " + 
                                  (isMut ? "mutable" : "immutable") + 
                                  " because it is already borrowed as " + 
                                  (loan.isMutable ? "mutable" : "immutable");
                diag_.error(loc, msg);
                return;
            }
        }
    }
    }
    
    // 2. Issue the loan
    if (!scopeStack_.empty()) scopeStack_.back().push_back(Loan{nextLoanId_++, place, isMut, loc});
}

void BorrowChecker::checkAccess(const Place& place, bool isMut, SourceLocation loc) {
    for (const auto& scope : scopeStack_) {
        for (const auto& loan : scope) {
        // If we are modifying or reading a place that overlaps with a loan.
        if (loan.place.overlapsWith(place)) {
            // If the access is mutable, or the active loan is mutable, it's a conflict
            if (isMut || loan.isMutable) {
                std::string msg = "Cannot " + std::string(isMut ? "mutate" : "access") + 
                                  " '" + place.toString() + "' because it is currently borrowed";
                diag_.error(loc, msg);
            }
        }
    }
    }
}

} // namespace fl
