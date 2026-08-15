// =============================================================================
// src/MiddleEnd/LifetimeConstraintSolver.cpp
// Sprint 7 — S7.1: Lifetime Constraint Graph Solver Implementation
// =============================================================================

#include "mellis/MiddleEnd/LifetimeConstraintSolver.h"
#include <unordered_set>
#include <algorithm>
#include <cassert>

namespace fl {

// ─────────────────────────────────────────────────────────────────────────────
// LifetimeUnionFind

void LifetimeUnionFind::ensureExists(LifetimeId id) {
    if (parent_.find(id) == parent_.end()) {
        parent_[id] = id;
        rank_[id]   = 0;
    }
}

LifetimeId LifetimeUnionFind::find(LifetimeId id) {
    ensureExists(id);
    // Path compression
    if (parent_[id] != id)
        parent_[id] = find(parent_[id]);
    return parent_[id];
}

void LifetimeUnionFind::unite(LifetimeId a, LifetimeId b) {
    LifetimeId ra = find(a);
    LifetimeId rb = find(b);
    if (ra == rb) return;

    // Union by rank
    if (rank_[ra] < rank_[rb]) std::swap(ra, rb);
    parent_[rb] = ra;
    if (rank_[ra] == rank_[rb]) rank_[ra]++;
}

// ─────────────────────────────────────────────────────────────────────────────
// LifetimeConstraintSolver

void LifetimeConstraintSolver::addEqual(Lifetime a, Lifetime b) {
    // Register lifetimes for tracking
    allLifetimes_.push_back(a);
    allLifetimes_.push_back(b);

    // Ensure they exist in union-find before merging
    unionFind_.find(a.id);
    unionFind_.find(b.id);
    equalConstraints_.push_back({a, LifetimeRelation::Equal, b});
    unionFind_.unite(a.id, b.id);
}

void LifetimeConstraintSolver::addOutlives(Lifetime longer, Lifetime shorter) {
    allLifetimes_.push_back(longer);
    allLifetimes_.push_back(shorter);
    // Ensure in union-find (as singleton equivalence classes if not seen yet)
    unionFind_.find(longer.id);
    unionFind_.find(shorter.id);

    outlivesEdges_.push_back(OutlivesEdge{
        longer.id, shorter.id,
        LifetimeConstraint{longer, LifetimeRelation::Outlives, shorter}
    });
}

void LifetimeConstraintSolver::reset() {
    unionFind_       = LifetimeUnionFind{};
    equalConstraints_.clear();
    outlivesEdges_.clear();
    allLifetimes_.clear();
}

std::vector<LifetimeConstraintSolver::OutlivesEdge>
LifetimeConstraintSolver::normalizeEdges() {
    std::vector<OutlivesEdge> normalized;
    for (auto& edge : outlivesEdges_) {
        LifetimeId rl = unionFind_.find(edge.longer);
        LifetimeId rs = unionFind_.find(edge.shorter);
        if (rl == rs) continue; // Same equivalence class — trivially satisfied
        normalized.push_back({rl, rs, edge.original});
    }
    return normalized;
}

std::vector<LifetimeConflict>
LifetimeConstraintSolver::detectCycles(const std::vector<OutlivesEdge>& edges) {
    // Collect all representative nodes
    std::unordered_set<LifetimeId> nodeSet;
    for (auto& e : edges) {
        nodeSet.insert(e.longer);
        nodeSet.insert(e.shorter);
    }
    std::vector<LifetimeId> nodes(nodeSet.begin(), nodeSet.end());
    size_t n = nodes.size();
    if (n == 0) return {};

    // Index map
    std::unordered_map<LifetimeId, size_t> idx;
    for (size_t i = 0; i < n; ++i) idx[nodes[i]] = i;

    // Build adjacency: reachable[i][j] = true if i outlives j (directly or transitively)
    // Use Floyd-Warshall for full transitive closure
    std::vector<std::vector<bool>> reach(n, std::vector<bool>(n, false));
    // Also store the original edge for each direct connection (for conflict chain)
    std::vector<std::vector<const OutlivesEdge*>> directEdge(n, std::vector<const OutlivesEdge*>(n, nullptr));

    for (auto& e : edges) {
        size_t i = idx[e.longer];
        size_t j = idx[e.shorter];
        reach[i][j]      = true;
        directEdge[i][j] = &e;
    }

    // Floyd-Warshall transitive closure
    for (size_t k = 0; k < n; ++k)
        for (size_t i = 0; i < n; ++i)
            for (size_t j = 0; j < n; ++j)
                if (reach[i][k] && reach[k][j])
                    reach[i][j] = true;

    // Detect cycles: i outlives j AND j outlives i AND i != j
    std::vector<LifetimeConflict> conflicts;
    std::unordered_set<size_t> reported;

    for (size_t i = 0; i < n; ++i) {
        for (size_t j = 0; j < n; ++j) {
            if (i == j) continue;
            if (reach[i][j] && reach[j][i]) {
                // Cycle between nodes[i] and nodes[j]
                size_t key = i < j ? i * n + j : j * n + i;
                if (reported.count(key)) continue;
                reported.insert(key);

                LifetimeConflict conflict;
                // Primary: i→j
                if (directEdge[i][j]) {
                    conflict.primary = directEdge[i][j]->original;
                } else {
                    // Synthesize from transitive path
                    Lifetime lt_i, lt_j;
                    lt_i.id = nodes[i]; lt_i.kind = LifetimeKind::Named;
                    lt_j.id = nodes[j]; lt_j.kind = LifetimeKind::Named;
                    conflict.primary = {lt_i, LifetimeRelation::Outlives, lt_j};
                }
                // Chain: j→i (the reverse direction that causes the cycle)
                if (directEdge[j][i]) {
                    conflict.chain.push_back(directEdge[j][i]->original);
                } else {
                    Lifetime lt_i, lt_j;
                    lt_i.id = nodes[i]; lt_i.kind = LifetimeKind::Named;
                    lt_j.id = nodes[j]; lt_j.kind = LifetimeKind::Named;
                    conflict.chain.push_back({lt_j, LifetimeRelation::Outlives, lt_i});
                }
                conflicts.push_back(std::move(conflict));
            }
        }
    }
    return conflicts;
}

LifetimeSolveResult LifetimeConstraintSolver::solve() {
    // Phase 1: All Equal constraints already applied via addEqual → unionFind_
    // Phase 2: Normalize outlives edges to use representative IDs
    auto normalized = normalizeEdges();

    // Phase 3: Transitive closure + contradiction detection
    auto conflicts = detectCycles(normalized);

    LifetimeSolveResult result;
    result.satisfiable = conflicts.empty();
    result.conflicts   = std::move(conflicts);
    return result;
}

} // namespace fl
