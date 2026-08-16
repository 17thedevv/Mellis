// =============================================================================
// mellis/MiddleEnd/LifetimeConstraintSolver.h
//
// Sprint 7 — S7.1: Lifetime Constraint Graph Solver
//
// Design:
//   Phase 1: Normalization — Equal('a, 'b) → Union-Find merge.
//             Graph only contains Outlives between equivalence classes.
//   Phase 2: Transitive Closure — 'a⊇'b AND 'b⊇'c → 'a⊇'c
//   Phase 3: Contradiction Detection — cycle in strict partial order
//             ('a⊇'b AND 'b⊇'a AND 'a≠'b)
//
// Usage:
//   LifetimeConstraintSolver solver;
//   solver.addEqual(lt_a, lt_b);
//   solver.addOutlives(lt_a, lt_b);  // 'a : 'b
//   auto result = solver.solve();
//   if (!result.satisfiable) { /* emit diagnostics from result.conflicts */ }
// =============================================================================

#pragma once

#include "mellis/Core/FLType.h"
#include <unordered_map>
#include <vector>
#include <string>

namespace fl {

// ─────────────────────────────────────────────────────────────────────────────
// Conflict chain: one constraint that contributes to an unsatisfiable set

struct LifetimeConflict {
    LifetimeConstraint primary;                ///< The direct violating constraint
    std::vector<LifetimeConstraint> chain;     ///< Transitive chain leading to conflict
};

struct LifetimeSolveResult {
    bool satisfiable = true;
    std::vector<LifetimeConflict> conflicts;   ///< Populated when !satisfiable
};

// ─────────────────────────────────────────────────────────────────────────────
// Union-Find for lifetime equality normalization

class LifetimeUnionFind {
public:
    /// Merge equivalence class of a with b (Equal constraint)
    void unite(LifetimeId a, LifetimeId b);
    /// Find canonical representative of a lifetime's equivalence class
    LifetimeId find(LifetimeId id);
    /// Check if two lifetimes are in the same equivalence class
    bool same(LifetimeId a, LifetimeId b) { return find(a) == find(b); }

private:
    std::unordered_map<LifetimeId, LifetimeId> parent_;
    std::unordered_map<LifetimeId, int> rank_;

    void ensureExists(LifetimeId id);
};

// ─────────────────────────────────────────────────────────────────────────────
// Main solver

class LifetimeConstraintSolver {
public:
    /// Add an equality constraint: 'a == 'b
    void addEqual(Lifetime a, Lifetime b);

    /// Add an outlives constraint: 'a : 'b  (a outlives b, a is longer-lived)
    void addOutlives(Lifetime longer, Lifetime shorter);

    /// Run the solver. Returns satisfiable=true if no contradiction found.
    /// Contradiction: 'a⊇'b AND 'b⊇'a AND 'a≠'b (strict cycle).
    LifetimeSolveResult solve();

    /// Reset all constraints (for reuse across functions)
    void reset();

private:
    struct OutlivesEdge {
        LifetimeId longer;           ///< representative after union-find
        LifetimeId shorter;
        LifetimeConstraint original; ///< for diagnostics
    };

    LifetimeUnionFind unionFind_;
    std::vector<LifetimeConstraint> equalConstraints_;
    std::vector<OutlivesEdge> outlivesEdges_;
    std::vector<Lifetime> allLifetimes_;   ///< all lifetimes seen (for normalization)

    /// Normalize outlives edges to use equivalence class representatives
    std::vector<OutlivesEdge> normalizeEdges();

    /// Transitive closure + cycle detection on normalized edges
    std::vector<LifetimeConflict> detectCycles(const std::vector<OutlivesEdge>& edges);

    LifetimeId rep(const Lifetime& lt) {
        return unionFind_.find(lt.id);
    }
};

} // namespace fl
