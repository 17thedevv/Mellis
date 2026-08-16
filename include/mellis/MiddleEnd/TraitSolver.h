#pragma once

#include "mellis/Core/FLType.h"
#include <vector>
#include <unordered_map>
#include <functional>

namespace fl {

enum class GoalKind {
    Trait,
    Projection,
    MethodResolution,
    ObjectSafety
};

struct Goal {
    GoalKind kind;
    
    // For Trait & ObjectSafety Goals
    const Type* selfType;
    SymbolID traitId;
    std::vector<const Type*> genericArgs;
    
    // For Projection Goal (<T as Trait>::Assoc == U)
    std::string assocName;
    const Type* expectedType; 
    
    // For MethodResolution Goal
    std::string methodName;
};

// Represents an assumed bound in the environment (e.g. from `where T: Clone`)
struct TraitBound {
    const Type* selfType;
    SymbolID traitId;
    std::vector<const Type*> genericArgs;
};

// Represents a logical clause derived from an `impl` block
class ImplDeclNode;

struct TraitClause {
    std::vector<SymbolID> genericParamIds; // The T, U, etc. introduced in this impl block
    const Type* selfType;
    SymbolID traitId;
    std::vector<const Type*> genericArgs;
    std::vector<Goal> obligations; // Where bounds of the impl block
    const ImplDeclNode* implNode = nullptr;
    std::vector<const Type*> instantiatedArgs;
    std::unordered_map<std::string, const Type*> associatedBindings;
};

enum class SolverResult {
    Success,
    Failure,
    Ambiguous,
    Incomplete
};

struct Solution {
    SolverResult result;
    const ImplDeclNode* implNode = nullptr;
    std::vector<const Type*> instantiatedArgs;
    
    // For MethodResolution
    SymbolID methodId = kInvalidSymbolID;
    bool isDynamicDispatch = false;
    size_t vtableOffset = 0;

    // For Ambiguous Result
    std::vector<const ImplDeclNode*> ambiguousCandidates;
    std::vector<SymbolID> ambiguousMethods;
};

class TypeContext;

class SymbolTable;
class DiagnosticEngine;
class MethodResolver;

class TraitSolver {
public:
    explicit TraitSolver(TypeContext& ctx, SymbolTable& table, DiagnosticEngine& diag, const std::vector<const Type*>& typeTable, MethodResolver& methodResolver);

    // Register an impl block as a clause
    void addClause(TraitClause clause);

    // Register a local environment bound
    void addBound(TraitBound bound);

    // Clear local environment bounds (useful when changing scopes)
    void clearBounds();

    // Prove a goal
    Solution solve(const Goal& goal);

    // Resolve an associated type projection: <T as Trait>::AssocName → concrete Type*
    // Returns nullptr if not resolvable (e.g. T is still a GenericParam in a generic context).
    const Type* resolveProjection(const AssociatedTypeProjection* proj);

    // Same as above but for string-based lookup (Self::Item shorthand)
    const Type* resolveAssociatedType(const Type* selfType, SymbolID traitId, const std::string& assocName, std::function<const Type*(const TypeNode*)> evaluateFn = nullptr);

    // Substitute generic parameters with fresh inference variables
    const Type* instantiateWithFreshVars(const Type* type, const std::unordered_map<SymbolID, const Type*>& replacements);

private:
    TypeContext& ctx_;
    SymbolTable& table_;
    DiagnosticEngine& diag_;
    const std::vector<const Type*>& typeTable_;
    MethodResolver& methodResolver_;
    std::vector<TraitClause> clauses_;
    std::vector<TraitBound> env_;

    Solution solveRecursive(const Goal& goal, size_t depth);
    
    // Attempt to unify two types.
    bool unify(const Type* expected, const Type* actual);
};

} // namespace fl
