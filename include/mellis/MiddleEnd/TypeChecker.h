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


    struct MethodInfo {
        SymbolID methodId;
        const FunctionType* type;
    };

    bool resolveMethod(const Type* receiverType, const std::string& name, MethodInfo& outMethod, ModuleID callerModuleID = 0);

    struct MethodCandidate {
        enum Kind { Trait, Inherent };
        Kind kind;
        SymbolID traitId;
        const ImplDeclNode* inherentImplNode;
        SymbolID methodId; // Original method symbol ID (from TraitDecl or ImplDecl)
        const FunctionType* methodType; // Original method type
    };

    class MethodResolver {
        std::unordered_map<std::string, std::vector<MethodCandidate>> candidates;
    public:
        void addTraitMethod(const std::string& name, SymbolID traitId, SymbolID methodId, const FunctionType* type);
        void addInherentMethod(const std::string& name, const ImplDeclNode* implNode, SymbolID methodId, const FunctionType* type);
        
        bool probe(const Type* receiverType, const std::string& name, MethodInfo& outMethod, TraitSolver& solver, TypeContext& ctx, MonomorphizationEngine* monoEngine, SymbolTable& table, const std::vector<const Type*>& typeTable, ModuleID callerModuleID = 0);
    };

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
    mutable TraitSolver traitSolver_;
    MethodResolver methodResolver_;
    LifetimeConstraintSolver lifetimeSolver_; ///< S7.1: per-check() lifetime constraint accumulator
};


} // namespace fl
