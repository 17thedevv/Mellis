// =============================================================================
// mellis/MiddleEnd/TypeChecker.h
//
// TypeChecker - Evaluates and infers Semantic Types.
// =============================================================================
#pragma once
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/MLib/MLibMetadataCache.h"
#include "mellis/MiddleEnd/MethodResolver.h"
#include "mellis/MiddleEnd/LifetimeConstraintSolver.h"
#include <vector>
#include <unordered_map>
#include "mellis/MiddleEnd/TraitSolver.h"

namespace fl {

class MonomorphizationEngine;

class TypeChecker {
public:
    explicit TypeChecker(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, MonomorphizationEngine* monoEngine = nullptr);
    bool check(ASTNode* root, ModuleID currentModule = 0);
    const Type* typeOf(SymbolID id) const;
    std::function<const Type*(const TypeNode*)> evaluateASTFn;
    TypeContext& getContext() const { return ctx_; }
    void setMonomorphizationEngine(MonomorphizationEngine* engine) { monoEngine_ = engine; }

    // Attach an MLibMetadataCache so TypeChecker can resolve external symbols.
    // Must be called before check() if any external modules are loaded.
    void setMetadataCache(MLibMetadataCache* cache) { metadataCache_ = cache; }

    void registerImpl(const ImplDeclNode* implNode);
    bool implementsTrait(const Type* type, SymbolID traitId, const std::vector<const Type*>& genericArgs = {}) const;
    void registerMethod(SymbolID traitId, const std::string& name, SymbolID methodId, const FunctionType* type);
    
    const Type* resolveAssociatedType(const Type* selfType, SymbolID traitId, const std::string& assocName) {
        // Find the trait bounds if traitId is kInvalidSymbolID, but wait, the caller can just use the traitSolver_ directly!
        return traitSolver_.resolveAssociatedType(selfType, traitId, assocName);
    }

    MethodResolver& getMethodResolver() { return methodResolver_; }
    TraitSolver& getTraitSolver() { return traitSolver_; }
    LifetimeConstraintSolver& getLifetimeSolver() { return lifetimeSolver_; }

    const std::vector<const Type*>& getTypeTable() const { return typeTable_; }

private:
    SymbolTable& table_;
    DiagnosticEngine& diag_;
    TypeContext& ctx_;
    MonomorphizationEngine* monoEngine_;
    MLibMetadataCache* metadataCache_ = nullptr;
    std::vector<const Type*> typeTable_;
    MethodResolver methodResolver_;
    mutable TraitSolver traitSolver_;
    LifetimeConstraintSolver lifetimeSolver_; ///< S7.1: per-check() lifetime constraint accumulator
};


} // namespace fl
