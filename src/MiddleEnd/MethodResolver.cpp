#include "mellis/MiddleEnd/MethodResolver.h"
#include "mellis/Support/Diagnostic.h"
#include <iostream>

namespace fl {

void MethodResolver::addTraitMethod(const std::string& name, SymbolID traitId, SymbolID methodId, const FunctionType* type) {
    MethodCandidate cand;
    cand.traitId = traitId;
    cand.inherentImplNode = nullptr;
    cand.methodId = methodId;
    cand.methodType = type;
    candidates[name].push_back(cand);
}

void MethodResolver::addInherentMethod(const std::string& name, const ImplDeclNode* implNode, SymbolID methodId, const FunctionType* type) {
    MethodCandidate cand;
    cand.traitId = kInvalidSymbolID;
    cand.inherentImplNode = implNode;
    cand.methodId = methodId;
    cand.methodType = type;
    candidates[name].push_back(cand);
}

bool MethodResolver::probe(const Type* receiverType, const std::string& name, MethodInfo& outMethod, TraitSolver& solver, TypeContext& ctx, SymbolTable& table, const std::vector<const Type*>& typeTable, ModuleID callerModuleID, DiagnosticEngine* diag, SourceLocation callLoc) {
    if (!receiverType || receiverType->getKind() == TypeKind::Unknown) return false;
    
    std::cerr << "[DEBUG MethodResolver] Probing method '" << name << "' for receiverType: " << receiverType->toString() << std::endl;

    Goal goal;
    goal.kind = GoalKind::MethodResolution;
    goal.selfType = receiverType;
    goal.methodName = name;

    Solution sol = solver.solve(goal);
    
    if (sol.result == SolverResult::Ambiguous) {
        if (diag) {
            auto& d = diag->error(callLoc, "multiple implementations match this call", "E-TRAIT-AMBIGUOUS");
            for (auto mid : sol.ambiguousMethods) {
                const auto& sym = table.getSymbol(mid);
                SourceLocation mLoc = sym.decl ? sym.decl->loc : SourceLocation();
                d.addNote(mLoc, "candidate method");
            }
        }
        return false;
    }
    
    if (sol.result == SolverResult::Overflow) {
        if (diag) {
            diag->error(callLoc, "trait resolution exceeded the recursion limit", "E-TRAIT-SOLVER-OVERFLOW");
        }
        return false;
    }
    
    if (sol.result == SolverResult::Success) {
        // Reconstruct MethodInfo from the successful candidate in our own map
        auto it = candidates.find(name);
        if (it != candidates.end()) {
            for (const auto& cand : it->second) {
                if (cand.methodId == sol.methodId) {
                    outMethod.id = cand.methodId;
                    outMethod.isTraitMethod = (cand.traitId != kInvalidSymbolID);
                    outMethod.traitId = cand.traitId;
                    outMethod.implNode = sol.implNode;
                    
                    const Type* substitutedMethodType = cand.methodType;
                    if (outMethod.isTraitMethod && cand.methodType && !cand.methodType->paramTypes.empty()) {
                        const Type* firstParam = cand.methodType->paramTypes[0];
                        const Type* stripped = firstParam;
                        if (auto* ref = dynamic_cast<const ReferenceType*>(stripped)) stripped = ref->pointee;
                        else if (auto* ptr = dynamic_cast<const PointerType*>(stripped)) stripped = ptr->pointee;
                        
                        if (auto* gp = dynamic_cast<const GenericParamType*>(stripped)) {
                            std::unordered_map<SymbolID, const Type*> mapping;
                            const Type* baseType = receiverType;
                            if (auto* r = dynamic_cast<const ReferenceType*>(baseType)) baseType = r->pointee;
                            else if (auto* p = dynamic_cast<const PointerType*>(baseType)) baseType = p->pointee;
                            mapping[gp->paramId] = baseType;
                            substitutedMethodType = ctx.substitute(cand.methodType, mapping);
                        }
                    }

                    outMethod.type = dynamic_cast<const FunctionType*>(substitutedMethodType);
                    if (!outMethod.type) outMethod.type = cand.methodType;
                    return true;
                }
            }
        }
    }
    
    return false;
}

} // namespace fl
