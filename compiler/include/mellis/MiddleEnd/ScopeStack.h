// =============================================================================
// mellis/MiddleEnd/ScopeStack.h
//
// ScopeStack — tracks the ACTIVE scope chain during a single AST traversal.
//
// Separation from SymbolTable (design decision):
//   SymbolTable is a PERSISTENT data store — it owns all Symbol and Scope
//   objects and is queried by EVERY compiler pass.
//
//   ScopeStack is TRANSIENT — it exists only while one pass is traversing
//   the AST. When Resolver finishes, ScopeStack is discarded.
//
//   This separation means:
//   - Multiple passes can each have their own ScopeStack pointing into
//     the same SymbolTable without interfering with each other.
//   - SymbolTable has zero dependency on traversal state.
//   - ScopeStack can be instantiated on the stack (no heap allocation
//     for the stack object itself).
//
// Invariant: stack_ is never empty while a Resolver pass is running
// (the global scope is pushed before the first statement and popped last).
// =============================================================================

#pragma once
#include "mellis/Core/Types.h"
#include <vector>
#include <cassert>

namespace fl {

class ScopeStack {
public:
    ScopeStack() = default;

    // Not copyable — copying active traversal state makes no sense.
    ScopeStack(const ScopeStack&) = delete;
    ScopeStack& operator=(const ScopeStack&) = delete;

    // Movable — allows passing between helpers if needed.
    ScopeStack(ScopeStack&&) = default;
    ScopeStack& operator=(ScopeStack&&) = default;

    // ── Core operations ──────────────────────────────────────────────────────

    /// Push a scope (created by SymbolTable) onto the active chain.
    /// Called when entering a block, function, module, etc.
    void push(ScopeID id) {
        stack_.push_back(id);
    }

    /// Pop the innermost scope.
    /// Called when exiting a block, function, module, etc.
    /// Precondition: stack_ is not empty.
    void pop() {
        assert(!stack_.empty() && "ScopeStack::pop() called on empty stack");
        stack_.pop_back();
    }

    /// The innermost (currently active) scope.
    /// All new declarations go here. All name lookups start here.
    /// Precondition: stack_ is not empty.
    ScopeID current() const {
        assert(!stack_.empty() && "ScopeStack::current() called on empty stack");
        return stack_.back();
    }

    // ── Observers ────────────────────────────────────────────────────────────

    bool   isEmpty() const { return stack_.empty(); }
    size_t depth()   const { return stack_.size(); }

    /// Read-only access to the full scope chain (innermost last).
    /// Useful for diagnostics ("declared inside while inside if inside fn …").
    const std::vector<ScopeID>& chain() const { return stack_; }

private:
    // Scopes are stored innermost-last (back = current).
    // vector is preferred over std::stack because:
    //   - chain() access needed for diagnostics
    //   - amortized O(1) push/pop
    //   - cache-friendly layout
    std::vector<ScopeID> stack_;
};

} // namespace fl
