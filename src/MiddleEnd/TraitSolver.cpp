#include "mellis/MiddleEnd/TraitSolver.h"
#include "mellis/AST/DeclNode.h"
#include <iostream>
#include <cassert>
#include <unordered_map>

namespace fl {

TraitSolver::TraitSolver(TypeContext& ctx) : ctx_(ctx) {}

void TraitSolver::addClause(TraitClause clause) {
    clauses_.push_back(std::move(clause));
}

void TraitSolver::addBound(TraitBound bound) {
    env_.push_back(std::move(bound));
}

void TraitSolver::clearBounds() {
    env_.clear();
}

const Type* TraitSolver::instantiateWithFreshVars(const Type* type, const std::unordered_map<SymbolID, const Type*>& replacements) {
    if (!type) return nullptr;

    if (auto* gp = dynamic_cast<const GenericParamType*>(type)) {
        auto it = replacements.find(gp->paramId);
        if (it != replacements.end()) return it->second;
        return type;
    }

    if (auto* ref = dynamic_cast<const ReferenceType*>(type)) {
        return ctx_.getReferenceType(instantiateWithFreshVars(ref->pointee, replacements), ref->isMutable);
    }
    
    if (auto* ptr = dynamic_cast<const PointerType*>(type)) {
        return ctx_.getPointerType(instantiateWithFreshVars(ptr->pointee, replacements), ptr->isMutable);
    }
    
    if (auto* tup = dynamic_cast<const TupleType*>(type)) {
        std::vector<const Type*> elems;
        for (auto* el : tup->elements) {
            elems.push_back(instantiateWithFreshVars(el, replacements));
        }
        return ctx_.getTupleType(elems);
    }
    
    if (auto* arr = dynamic_cast<const ArrayType*>(type)) {
        return ctx_.getArrayType(instantiateWithFreshVars(arr->elementType, replacements), arr->length);
    }
    
    if (auto* sl = dynamic_cast<const SliceType*>(type)) {
        return ctx_.getSliceType(instantiateWithFreshVars(sl->elementType, replacements));
    }
    
    if (auto* st = dynamic_cast<const StructType*>(type)) {
        if (st->genericArgs.empty()) return type;
        std::vector<const Type*> args;
        for (auto* arg : st->genericArgs) {
            args.push_back(instantiateWithFreshVars(arg, replacements));
        }
        return ctx_.getStructType(st->structSymbolId, args);
    }
    
    if (auto* en = dynamic_cast<const EnumType*>(type)) {
        if (en->genericArgs.empty()) return type;
        std::vector<const Type*> args;
        for (auto* arg : en->genericArgs) {
            args.push_back(instantiateWithFreshVars(arg, replacements));
        }
        return ctx_.getEnumType(en->enumSymbolId, args);
    }

    if (auto* fn = dynamic_cast<const FunctionType*>(type)) {
        std::vector<const Type*> paramTypes;
        for (auto* pt : fn->paramTypes) {
            paramTypes.push_back(instantiateWithFreshVars(pt, replacements));
        }
        const Type* retType = instantiateWithFreshVars(fn->returnType, replacements);
        return ctx_.getFunctionType(fn->paramNames, paramTypes, retType, fn->isCallSite, fn->isVariadic);
    }
    
    if (auto* proj = dynamic_cast<const AssociatedTypeProjection*>(type)) {
        const Type* newSelf = instantiateWithFreshVars(proj->selfType, replacements);
        return ctx_.getAssociatedProjection(newSelf, proj->traitId, proj->assocName);
    }

    return type;
}

Solution TraitSolver::solve(const TraitGoal& goal) {
    // Top-level solve
    return solveRecursive(goal, 0);
}

bool TraitSolver::unify(const Type* expected, const Type* actual) {
    if (!expected || !actual) return false;
    
    expected = ctx_.unificationTable.deepResolve(expected, ctx_);
    actual = ctx_.unificationTable.deepResolve(actual, ctx_);
    
    if (expected == actual) return true;
    
    if (auto* inf = dynamic_cast<const InferenceVarType*>(expected)) {
        // Simple occurs check could go here
        ctx_.unificationTable.unify(inf->varId, actual);
        return true;
    }
    
    if (auto* inf = dynamic_cast<const InferenceVarType*>(actual)) {
        ctx_.unificationTable.unify(inf->varId, expected);
        return true;
    }
    
    // Structural unification
    if (auto* expRef = dynamic_cast<const ReferenceType*>(expected)) {
        if (auto* actRef = dynamic_cast<const ReferenceType*>(actual)) {
            if (expRef->isMutable != actRef->isMutable) return false;
            return unify(expRef->pointee, actRef->pointee);
        }
    }
    
    if (auto* expPtr = dynamic_cast<const PointerType*>(expected)) {
        if (auto* actPtr = dynamic_cast<const PointerType*>(actual)) {
            if (expPtr->isMutable != actPtr->isMutable) return false;
            return unify(expPtr->pointee, actPtr->pointee);
        }
    }
    
    if (auto* expStruct = dynamic_cast<const StructType*>(expected)) {
        if (auto* actStruct = dynamic_cast<const StructType*>(actual)) {
            if (expStruct->structSymbolId != actStruct->structSymbolId) {
                if (expStruct->originalTemplateId != kInvalidSymbolID && expStruct->originalTemplateId == actStruct->structSymbolId) {
                    if (expStruct->specializedArgs.size() != actStruct->genericArgs.size()) return false;
                    for (size_t i = 0; i < expStruct->specializedArgs.size(); ++i) {
                        if (!unify(expStruct->specializedArgs[i], actStruct->genericArgs[i])) return false;
                    }
                    return true;
                }
                if (actStruct->originalTemplateId != kInvalidSymbolID && actStruct->originalTemplateId == expStruct->structSymbolId) {
                    if (actStruct->specializedArgs.size() != expStruct->genericArgs.size()) return false;
                    for (size_t i = 0; i < actStruct->specializedArgs.size(); ++i) {
                        if (!unify(actStruct->specializedArgs[i], expStruct->genericArgs[i])) return false;
                    }
                    return true;
                }
                return false;
            }
            if (expStruct->genericArgs.size() != actStruct->genericArgs.size()) return false;
            for (size_t i = 0; i < expStruct->genericArgs.size(); ++i) {
                if (!unify(expStruct->genericArgs[i], actStruct->genericArgs[i])) return false;
            }
            return true;
        }
    }
    
    if (auto* expEnum = dynamic_cast<const EnumType*>(expected)) {
        if (auto* actEnum = dynamic_cast<const EnumType*>(actual)) {
            if (expEnum->enumSymbolId != actEnum->enumSymbolId) return false;
            if (expEnum->genericArgs.size() != actEnum->genericArgs.size()) return false;
            for (size_t i = 0; i < expEnum->genericArgs.size(); ++i) {
                if (!unify(expEnum->genericArgs[i], actEnum->genericArgs[i])) return false;
            }
            return true;
        }
    }
    
    if (auto* expTup = dynamic_cast<const TupleType*>(expected)) {
        if (auto* actTup = dynamic_cast<const TupleType*>(actual)) {
            if (expTup->elements.size() != actTup->elements.size()) return false;
            for (size_t i = 0; i < expTup->elements.size(); ++i) {
                if (!unify(expTup->elements[i], actTup->elements[i])) return false;
            }
            return true;
        }
    }

    if (auto* expSlice = dynamic_cast<const SliceType*>(expected)) {
        if (auto* actSlice = dynamic_cast<const SliceType*>(actual)) {
            return unify(expSlice->elementType, actSlice->elementType);
        }
    }

    if (auto* expArray = dynamic_cast<const ArrayType*>(expected)) {
        if (auto* actArray = dynamic_cast<const ArrayType*>(actual)) {
            if (expArray->length != actArray->length) return false;
            return unify(expArray->elementType, actArray->elementType);
        }
    }

    return false;
}

Solution TraitSolver::solveRecursive(const TraitGoal& goal, size_t depth) {
    if (depth > 64) {
        // Prevent infinite recursion (e.g. self-referential impls)
        return {SolverResult::Failed, nullptr};
    }

    std::cerr << "[DEBUG TraitSolver] Solving goal: self=" << goal.selfType->toString() << " traitId=" << goal.traitId << std::endl;
    // 1. Try to satisfy from environment (assumed bounds)
    for (const auto& bound : env_) {
        std::cerr << "[DEBUG TraitSolver]   Checking env bound: self=" << bound.selfType->toString() << " traitId=" << bound.traitId << std::endl;
        if (bound.traitId != goal.traitId) continue;
        if (bound.genericArgs.size() != goal.genericArgs.size()) continue;

        auto snap = ctx_.unificationTable.snapshot();
        
        bool ok = unify(bound.selfType, goal.selfType);
        for (size_t i = 0; ok && i < goal.genericArgs.size(); ++i) {
            ok = unify(bound.genericArgs[i], goal.genericArgs[i]);
        }

        if (ok) {
            // Environment matches completely!
            std::cerr << "[DEBUG TraitSolver]     -> MATCHED env bound!" << std::endl;
            return {SolverResult::Proven, nullptr};
        } else {
            ctx_.unificationTable.rollback(snap);
        }
    }

    // 2. Try to satisfy from global clauses (impl blocks)
    int successfulCandidates = 0;

    for (const auto& clause : clauses_) {
        if (clause.traitId != goal.traitId) continue;
        if (clause.genericArgs.size() != goal.genericArgs.size()) continue;

        auto snap = ctx_.unificationTable.snapshot();

        // Instantiate the clause with fresh inference variables
        std::unordered_map<SymbolID, const Type*> freshVars;
        for (SymbolID id : clause.genericParamIds) {
            uint32_t infId = ctx_.unificationTable.newVar();
            freshVars[id] = ctx_.getInferenceVar(infId);
        }
        
        const Type* instSelf = instantiateWithFreshVars(clause.selfType, freshVars);
        bool ok = unify(instSelf, goal.selfType);
        
        for (size_t i = 0; ok && i < goal.genericArgs.size(); ++i) {
            const Type* instArg = instantiateWithFreshVars(clause.genericArgs[i], freshVars);
            ok = unify(instArg, goal.genericArgs[i]);
        }

        if (ok) {
            // Unification succeeded! Now we must prove all obligations
            bool allProven = true;
            for (const auto& obl : clause.obligations) {
                TraitGoal instObl = obl;
                instObl.selfType = instantiateWithFreshVars(obl.selfType, freshVars);
                for (size_t i = 0; i < instObl.genericArgs.size(); ++i) {
                    instObl.genericArgs[i] = instantiateWithFreshVars(obl.genericArgs[i], freshVars);
                }
                
                if (solveRecursive(instObl, depth + 1).result != SolverResult::Proven) {
                    allProven = false;
                    break;
                }
            }
            if (allProven) {
                successfulCandidates++;
                // Stop early on first match?
                // Real trait solvers might check for ambiguity if multiple impls match,
                // but we will just return Proven for now to keep it simpler.
                
                std::vector<const Type*> instArgs;
                for (SymbolID id : clause.genericParamIds) {
                    instArgs.push_back(ctx_.unificationTable.deepResolve(freshVars[id], ctx_));
                }
                return {SolverResult::Proven, clause.implNode, instArgs};

            }
        }
        
        ctx_.unificationTable.rollback(snap);
    }

    if (successfulCandidates == 0) return {SolverResult::Failed, nullptr};
    if (successfulCandidates == 1) return {SolverResult::Proven, nullptr};
    return {SolverResult::Ambiguous, nullptr};
}

const Type* TraitSolver::resolveAssociatedType(const Type* selfType, SymbolID traitId, const std::string& assocName, std::function<const Type*(const TypeNode*)> evaluateFn) {
    if (!selfType) return nullptr;

    printf("[DEBUG TraitSolver] resolveAssociatedType: selfType=%s, traitId=%d, assocName=%s\n", selfType->toString().c_str(), traitId, assocName.c_str());

    // Search through all registered impl clauses for a matching impl
    for (const auto& clause : clauses_) {
        if (traitId != kInvalidSymbolID && clause.traitId != traitId) continue;
        if (!clause.implNode) continue;

        // Try to unify selfType with clause.selfType
        auto snap = ctx_.unificationTable.snapshot();
        std::unordered_map<SymbolID, const Type*> freshVars;
        for (SymbolID id : clause.genericParamIds) {
            uint32_t infId = ctx_.unificationTable.newVar();
            freshVars[id] = ctx_.getInferenceVar(infId);
        }

        const Type* instSelf = instantiateWithFreshVars(clause.selfType, freshVars);
        bool unified = unify(instSelf, selfType);

        printf("[DEBUG TraitSolver]   trying clause with selfType=%s (instSelf=%s): unified=%d\n", clause.selfType->toString().c_str(), instSelf->toString().c_str(), unified);

        if (unified) {
            // Look up the associated type binding in this impl
            for (const auto& at : clause.implNode->associatedTypes) {
                if (at && std::string(at->name) == assocName && at->aliasedType) {
                    if (evaluateFn) {
                        const Type* resolved = evaluateFn(at->aliasedType.get());
                        if (resolved) {
                            resolved = instantiateWithFreshVars(resolved, freshVars);
                            resolved = ctx_.unificationTable.deepResolve(resolved, ctx_);
                            ctx_.unificationTable.commit(snap);
                            printf("[DEBUG TraitSolver]     -> resolved to %s\n", resolved->toString().c_str());
                            return resolved;
                        }
                    }
                    ctx_.unificationTable.commit(snap);
                    printf("[DEBUG TraitSolver]     -> evaluateFn is null or returned null\n");
                    return nullptr; // Resolved by TypeChecker via ImplDeclNode AST
                }
            }
        }

        ctx_.unificationTable.rollback(snap);
    }
    printf("[DEBUG TraitSolver]   -> no matching clause found\n");
    return nullptr;
}

const Type* TraitSolver::resolveProjection(const AssociatedTypeProjection* proj) {
    if (!proj) return nullptr;
    return resolveAssociatedType(proj->selfType, proj->traitId, proj->assocName);
}

} // namespace fl
