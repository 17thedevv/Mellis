#include "mellis/MiddleEnd/Semantic/MoveAnalyzer.h"

namespace fl {

Place MoveAnalyzer::resolvePlace(const mvir::Operand& op, const MoveStateData& state) const {
    if (auto* locId = mvir::getLocalIf(op)) {
        auto it = state.placeMap.find(locId->name);
        if (it != state.placeMap.end()) {
            return it->second;
        }
    }
    return Place(op);
}

void MoveAnalyzer::checkAccess(const Place& place, SourceLocation loc, const MoveStateData& state) {
    auto it = state.stateMap.find(place.toString());
    if (it != state.stateMap.end()) {
        if (it->second == MoveState::Moved) {
            diag_.error(loc, "Use of moved value: '" + place.toString() + "'");
            hasError_ = true;
        } else if (it->second == MoveState::PartiallyMoved) {
            diag_.error(loc, "Use of partially moved value: '" + place.toString() + "'");
            hasError_ = true;
        }
    }
}

void MoveAnalyzer::initEntryState(const mvir::Function& func, MoveStateData& state) {
    // Parameters are valid (not moved).
    for (const auto& param : func.params) {
        Place paramPlace(mvir::Operand(mvir::Place(param.id)));
        state.stateMap[paramPlace.toString()] = MoveState::Valid;
    }
}

bool MoveAnalyzer::merge(MoveStateData& dest, const MoveStateData& src) {
    bool changed = false;
    for (const auto& pa : src.placeMap) {
        if (dest.placeMap.find(pa.first) == dest.placeMap.end()) {
            dest.placeMap[pa.first] = pa.second;
            changed = true;
        }
    }
    
    for (const auto& ps : src.stateMap) {
        auto it = dest.stateMap.find(ps.first);
        if (it != dest.stateMap.end()) {
            if (ps.second != it->second) {
                // If one path is Moved and the other is Valid, it becomes PartiallyMoved.
                // If one is Moved and the other is PartiallyMoved, it becomes PartiallyMoved.
                if (it->second != MoveState::PartiallyMoved) {
                    dest.stateMap[ps.first] = MoveState::PartiallyMoved;
                    changed = true;
                }
            }
        } else {
            dest.stateMap[ps.first] = ps.second;
            changed = true;
        }
    }
    return changed;
}

void MoveAnalyzer::transferInstruction(const mvir::Instruction& inst, MoveStateData& state) {
    SourceLocation fakeLoc{0, 0, 0}; 

    if (auto* localInst = dynamic_cast<const mvir::LocalInst*>(&inst)) {
        Place localPlace(mvir::Operand(mvir::Place(localInst->dest)));
        state.stateMap[localPlace.toString()] = MoveState::Valid;
    }
    else if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(&inst)) {
        Place basePlace = resolvePlace(borrow->base, state);
        checkAccess(basePlace, fakeLoc, state);
        // Do not map borrow to derefPlace, so it remains an independent temporary.
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(&inst)) {
        Place srcPlace = resolvePlace(load->ptr, state);
        checkAccess(srcPlace, fakeLoc, state);
        
        // Lazy move: Map the loaded value to the source place. It will be moved only if consumed.
        state.placeMap[load->dest.name] = srcPlace;
        
        Place localPlace2(mvir::Operand(mvir::Place(load->dest)));
        state.stateMap[localPlace2.toString()] = MoveState::Valid;
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(&inst)) {
        Place destPlace = resolvePlace(store->ptr, state);
        
        if (auto* locId = mvir::getLocalIf(store->value)) {
            Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
            checkAccess(srcPlace, fakeLoc, state);
            if (!store->type->isCopy()) {
                state.stateMap[srcPlace.toString()] = MoveState::Moved;
            }
        }
        state.stateMap[destPlace.toString()] = MoveState::Valid; // Dest becomes valid again after store
    }
    else if (auto* call = dynamic_cast<const mvir::CallInst*>(&inst)) {
        for (size_t i = 0; i < call->args.size(); ++i) {
            auto& arg = call->args[i];
            if (auto* locId = mvir::getLocalIf(arg)) {
                Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
                checkAccess(srcPlace, fakeLoc, state);
                
                bool isCopy = false;
                if (call->funcType && i < call->funcType->paramTypes.size()) {
                    isCopy = call->funcType->paramTypes[i]->isCopy();
                }
                
                if (!isCopy) {
                    state.stateMap[srcPlace.toString()] = MoveState::Moved;
                }
            }
        }
        if (call->dest) {
            Place destPlace(mvir::Operand(mvir::Place(*call->dest)));
            state.stateMap[destPlace.toString()] = MoveState::Valid;
        }
    }
    else if (auto* vcall = dynamic_cast<const mvir::VirtualCallInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(vcall->receiver)) {
            Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
            checkAccess(srcPlace, fakeLoc, state);
            // Receivers are usually references. We assume method calls implicitly reborrow if they are references.
            // For MVP, we'll assume virtual call receiver isn't consumed (it's a method call).
        }
        for (size_t i = 0; i < vcall->args.size(); ++i) {
            auto& arg = vcall->args[i];
            if (auto* locId = mvir::getLocalIf(arg)) {
                Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
                checkAccess(srcPlace, fakeLoc, state);
                
                bool isCopy = false;
                if (vcall->methodType && i < vcall->methodType->paramTypes.size()) {
                    isCopy = vcall->methodType->paramTypes[i]->isCopy();
                }
                if (!isCopy) {
                    state.stateMap[srcPlace.toString()] = MoveState::Moved;
                }
            }
        }
        if (vcall->dest) {
            Place destPlace(mvir::Operand(mvir::Place(*vcall->dest)));
            state.stateMap[destPlace.toString()] = MoveState::Valid;
        }
    }
    else if (auto* ret = dynamic_cast<const mvir::CastInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ret->value)) {
            Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
            checkAccess(srcPlace, fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(ret->dest)));
        state.stateMap[destPlace.toString()] = MoveState::Valid;
    }
    else if (auto* un = dynamic_cast<const mvir::UnaryInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(un->operand)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(un->dest)));
        state.stateMap[destPlace.toString()] = MoveState::Valid;
    }
    else if (auto* alu = dynamic_cast<const mvir::AluInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(alu->left)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        if (auto* locId = mvir::getLocalIf(alu->right)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(alu->dest)));
        state.stateMap[destPlace.toString()] = MoveState::Valid;
    }
    else if (auto* ext = dynamic_cast<const mvir::ExtractInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ext->base)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(ext->dest)));
        state.stateMap[destPlace.toString()] = MoveState::Valid;
    }
    else if (auto* tag = dynamic_cast<const mvir::TagInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(tag->base)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(tag->dest)));
        state.stateMap[destPlace.toString()] = MoveState::Valid;
    }
    else if (auto* var = dynamic_cast<const mvir::VariantInst*>(&inst)) {
        for (const auto& arg : var->args) {
            if (auto* locId = mvir::getLocalIf(arg)) {
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
            }
        }
        Place destPlace(mvir::Operand(mvir::Place(var->dest)));
        state.stateMap[destPlace.toString()] = MoveState::Valid;
    }
}

void MoveAnalyzer::transferTerminator(const mvir::Terminator& term, MoveStateData& state) {
    SourceLocation fakeLoc{0, 0, 0}; 
    if (auto* br = dynamic_cast<const mvir::BranchTerm*>(&term)) {
        if (auto* locId = mvir::getLocalIf(br->condition)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
    } else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(&term)) {
        if (auto* locId = mvir::getLocalIf(sw->condition)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
    } else if (auto* ret = dynamic_cast<const mvir::RetTerm*>(&term)) {
        if (ret->value) {
            if (auto* locId = mvir::getLocalIf((*ret->value))) {
                Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
                checkAccess(srcPlace, fakeLoc, state);
                // Returned values are always moved out
                state.stateMap[srcPlace.toString()] = MoveState::Moved;
            }
        }
    }
}

} // namespace fl
