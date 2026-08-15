#pragma once

#include "mellis/Core/FLType.h"
#include <vector>
#include <unordered_map>
#include <functional>

namespace fl {

// Represents a goal we want to prove: e.g. T: Clone<Args...>
struct TraitGoal {
    const Type* selfType;
    SymbolID traitId;
    std::vector<const Type*> genericArgs;
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
    std::vector<TraitGoal> obligations; // Where bounds of the impl block
    const ImplDeclNode* implNode = nullptr;
    std::vector<const Type*> instantiatedArgs;
};

enum class SolverResult {
    Proven,
    Failed,
    Ambiguous
};

struct Solution {
    SolverResult result;
    const ImplDeclNode* implNode = nullptr;
    std::vector<const Type*> instantiatedArgs;
};

class TypeContext;

class TraitSolver {
public:
    explicit TraitSolver(TypeContext& ctx);

    // Register an impl block as a clause
    void addClause(TraitClause clause);

    // Register a local environment bound
    void addBound(TraitBound bound);

    // Clear local environment bounds (useful when changing scopes)
    void clearBounds();

    // Prove a goal
    Solution solve(const TraitGoal& goal);

    // Resolve an associated type projection: <T as Trait>::AssocName → concrete Type*
    // Returns nullptr if not resolvable (e.g. T is still a GenericParam in a generic context).
    const Type* resolveProjection(const AssociatedTypeProjection* proj);

    // Same as above but for string-based lookup (Self::Item shorthand)
    const Type* resolveAssociatedType(const Type* selfType, SymbolID traitId, const std::string& assocName, std::function<const Type*(const TypeNode*)> evaluateFn = nullptr);

    // Substitute generic parameters with fresh inference variables
    const Type* instantiateWithFreshVars(const Type* type, const std::unordered_map<SymbolID, const Type*>& replacements);

private:
    TypeContext& ctx_;
    std::vector<TraitClause> clauses_;
    std::vector<TraitBound> env_;

    Solution solveRecursive(const TraitGoal& goal, size_t depth);
    
    // Attempt to unify two types.
    bool unify(const Type* expected, const Type* actual);
};

} // namespace fl
