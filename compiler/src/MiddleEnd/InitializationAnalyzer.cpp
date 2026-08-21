#include "mellis/MiddleEnd/Semantic/InitializationAnalyzer.h"

namespace fl {

Place InitializationAnalyzer::resolvePlace(const mvir::Operand& op, const InitStateData& state) const {
    if (auto* locId = mvir::getLocalIf(op)) {
        auto it = state.placeMap.find(locId->name);
        if (it != state.placeMap.end()) {
            return it->second;
        }
    }
    return Place(op);
}

void InitializationAnalyzer::checkAccess(const Place& place, SourceLocation loc, const InitStateData& state) {
    auto it = state.initStateMap.find(place.toString());
    if (it != state.initStateMap.end()) {
        if (it->second == InitState::Uninitialized) {
            bool hasFieldInit = false;
            std::string prefixDot = place.toString() + ".";
            std::string prefixBracket = place.toString() + "[";
            for (const auto& pair : state.initStateMap) {
                if ((pair.first.find(prefixDot) == 0 || pair.first.find(prefixBracket) == 0) && pair.second == InitState::Initialized) {
                    hasFieldInit = true;
                    break;
                }
            }
            if (!hasFieldInit) {
                diag_.error(loc, "Use of uninitialized variable: '" + place.toString() + "'");
                hasError_ = true;
            }
        }
    }
}

void InitializationAnalyzer::initEntryState(const mvir::Function& func, InitStateData& state) {
    for (const auto& param : func.params) {
        Place paramPlace(mvir::Operand(mvir::Place(param.id)));
        state.initStateMap[paramPlace.toString()] = InitState::Initialized;
    }
}

bool InitializationAnalyzer::merge(InitStateData& dest, const InitStateData& src) {
    bool changed = false;
    for (const auto& pa : src.placeMap) {
        if (dest.placeMap.find(pa.first) == dest.placeMap.end()) {
            dest.placeMap[pa.first] = pa.second;
            changed = true;
        }
    }
    
    for (const auto& ps : src.initStateMap) {
        auto it = dest.initStateMap.find(ps.first);
        if (it != dest.initStateMap.end()) {
            if (ps.second != it->second) {
                // If one path is Uninitialized and the other is Initialized, it becomes Uninitialized.
                if (it->second != InitState::Uninitialized) {
                    dest.initStateMap[ps.first] = InitState::Uninitialized;
                    changed = true;
                }
            }
        } else {
            dest.initStateMap[ps.first] = ps.second;
            changed = true;
        }
    }
    return changed;
}

void InitializationAnalyzer::transferInstruction(const mvir::Instruction& inst, InitStateData& state) {
    SourceLocation fakeLoc{0, 0, 0}; 

    if (auto* localInst = dynamic_cast<const mvir::LocalInst*>(&inst)) {
        Place localPlace(mvir::Operand(mvir::Place(localInst->dest)));
        state.initStateMap[localPlace.toString()] = InitState::Uninitialized;
    }
    else if (auto* heapAlloc = dynamic_cast<const mvir::HeapAllocInst*>(&inst)) {
        Place destPlace(mvir::Operand(mvir::Place(heapAlloc->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(&inst)) {
        Place basePlace = resolvePlace(borrow->base, state);
        checkAccess(basePlace, fakeLoc, state);
        Place derefPlace = basePlace;
        derefPlace.projections.push_back(Projection{ProjectionKind::Deref, ""});
        state.placeMap[borrow->dest.name] = derefPlace;
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(&inst)) {
        Place srcPlace = resolvePlace(load->ptr, state);
        checkAccess(srcPlace, fakeLoc, state);
        Place destPlace(load->dest);
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(&inst)) {
        Place destPlace = resolvePlace(store->ptr, state);
        
        if (auto* locId = mvir::getLocalIf(store->value)) {
            Place srcPlace = resolvePlace(mvir::Operand(mvir::Place(*locId)), state);
            checkAccess(srcPlace, fakeLoc, state);
        }
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* call = dynamic_cast<const mvir::CallInst*>(&inst)) {
        for (const auto& arg : call->args) {
            if (auto* locId = mvir::getLocalIf(arg)) {
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
            }
        }
        if (call->dest) {
            Place destPlace(mvir::Operand(mvir::Place(*call->dest)));
            state.initStateMap[destPlace.toString()] = InitState::Initialized;
        }
    }
    else if (auto* vcall = dynamic_cast<const mvir::VirtualCallInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(vcall->receiver)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        for (const auto& arg : vcall->args) {
            if (auto* locId = mvir::getLocalIf(arg)) {
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
            }
        }
        if (vcall->dest) {
            Place destPlace(mvir::Operand(mvir::Place(*vcall->dest)));
            state.initStateMap[destPlace.toString()] = InitState::Initialized;
        }
    }
    else if (auto* ret = dynamic_cast<const mvir::CastInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ret->value)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(ret->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* un = dynamic_cast<const mvir::UnaryInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(un->operand)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(un->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* alu = dynamic_cast<const mvir::AluInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(alu->left)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        if (auto* locId = mvir::getLocalIf(alu->right)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(alu->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* tupleExt = dynamic_cast<const mvir::TupleExtractInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(tupleExt->tuple)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(tupleExt->dest);
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* ext = dynamic_cast<const mvir::ExtractInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(ext->base)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(ext->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* tag = dynamic_cast<const mvir::TagInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(tag->base)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(tag->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* slice = dynamic_cast<const mvir::MakeSliceInst*>(&inst)) {
        if (auto* locId = mvir::getLocalIf(slice->basePtr)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        if (auto* locId = mvir::getLocalIf(slice->length)) {
            checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
        }
        Place destPlace(mvir::Operand(mvir::Place(slice->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
    else if (auto* var = dynamic_cast<const mvir::VariantInst*>(&inst)) {
        for (const auto& arg : var->args) {
            if (auto* locId = mvir::getLocalIf(arg)) {
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
            }
        }
        Place destPlace(mvir::Operand(mvir::Place(var->dest)));
        state.initStateMap[destPlace.toString()] = InitState::Initialized;
    }
}

void InitializationAnalyzer::transferTerminator(const mvir::Terminator& term, InitStateData& state) {
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
                checkAccess(resolvePlace(mvir::Operand(mvir::Place(*locId)), state), fakeLoc, state);
            }
        }
    }
}

} // namespace fl
