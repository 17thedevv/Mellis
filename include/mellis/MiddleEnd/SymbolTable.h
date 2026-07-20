// =============================================================================
// mellis/MiddleEnd/SymbolTable.h
//
// SymbolTable — the central registry of all resolved names.
//
// Design: pure data store, zero traversal logic.
//   - Owns all Symbol objects (arena_)
//   - Owns all Scope objects  (scopes_)
//   - Provides lookup, declaration, and direct-access APIs
//   - Has NO knowledge of the current traversal state (that's ScopeStack's job)
//
// Arena-based storage (vector<Symbol>, vector<Scope>):
//   - O(1) access by SymbolID / ScopeID (index into vector)
//   - Pointer stability NOT guaranteed across push_back
//     → always access via ID, never store raw Symbol* / Scope*
//   - Arena is never freed during compilation — symbols persist until
//     the end of the compilation unit (enables back-pointer `decl` in Symbol)
//
// Lookup semantics:
//   lookup(name, fromScope):
//     Walks the scope chain from fromScope toward the global scope.
//     Returns the FIRST match (innermost wins → shadowing support).
//
//   lookupInScope(name, scope):
//     Checks ONLY the given scope — no chain walk.
//     Used by Resolver to detect redeclarations in the current scope.
//
//   containsInScope(name, scope):
//     Fast boolean check — no optional overhead.
//     Used by Resolver before every declareSymbol() call.
// =============================================================================

#pragma once
#include "mellis/MiddleEnd/Symbol.h"
#include "mellis/MiddleEnd/ScopeStack.h"
#include <vector>
#include <optional>
#include <string_view>

namespace fl {

class SymbolTable {
public:
    // ── Construction ─────────────────────────────────────────────────────────

    /// Creates the SymbolTable and eagerly allocates the global scope (ScopeID 0).
    SymbolTable();

    // Not copyable — the arena owns ASTNode back-pointers that should not be
    // duplicated. Move is fine (e.g., returning from a factory function).
    SymbolTable(const SymbolTable&) = delete;
    SymbolTable& operator=(const SymbolTable&) = delete;
    SymbolTable(SymbolTable&&) = default;
    SymbolTable& operator=(SymbolTable&&) = default;

    // ── Scope Management ─────────────────────────────────────────────────────

    /// Create a new scope and return its ID.
    /// The new scope is recorded in scopes_ but NOT pushed onto any stack
    /// (ScopeStack manages the active stack separately).
    ///
    /// @param kind     The semantic kind of the new scope.
    /// @param parentId The enclosing scope. Pass kInvalidScopeID only for global.
    ScopeID createScope(ScopeKind kind, ScopeID parentId);

    /// Create a virtual scope for an external module loaded from .mlib.
    /// It does not have a parent scope (parentId = kInvalidScopeID).
    ScopeID createVirtualModuleScope(std::string_view moduleName);


    // ── Symbol Declaration ────────────────────────────────────────────────────

    /// Declare a new symbol in the given scope.
    ///
    /// Precondition: caller must check containsInScope() first.
    ///   Calling declareSymbol() for an already-declared name in the same scope
    ///   is a logic error — it will create a duplicate and corrupt the table.
    ///
    /// @param name     The source-text name of the symbol.
    /// @param kind     What kind of entity this is.
    /// @param scope    Which scope to declare in.
    /// @param location Source location of the declaration (for diagnostics).
    /// @param decl     Back-pointer to the declaring AST node (non-owning, nullable).
    /// @return         The new SymbolID.
    SymbolID declareSymbol(const Identifier& name,
                           SymbolKind        kind,
                           ScopeID           scope,
                           SourceLocation    location,
                           ASTNode*          decl = nullptr);

    /// Declare a symbol loaded from an external .mlib module.
    /// This bypasses AST nodes and sets up the external linkage fields.
    SymbolID declareExternalSymbol(const Identifier& name,
                                   SymbolKind        kind,
                                   ScopeID           virtualScope,
                                   uint32_t          mlibSymbolID,
                                   const uint8_t     moduleUUID[16]);


    // ── Lookup: Chain Walk ────────────────────────────────────────────────────

    /// Walk the scope chain from fromScope toward global, return first match.
    /// Returns nullopt if not found anywhere in the chain.
    /// This implements shadowing: inner declarations shadow outer ones.
    std::optional<SymbolID> lookup(std::string_view name, ScopeID fromScope) const;

    /// Overload accepting Identifier directly (avoids string construction).
    std::optional<SymbolID> lookup(const Identifier& name, ScopeID fromScope) const;

    // ── Lookup: Single Scope (no chain walk) ──────────────────────────────────

    /// Check if `name` is declared in exactly `scope` (no chain walk).
    /// O(1) unordered_map lookup.
    /// Primary use: redeclaration detection in Resolver::declare().
    bool containsInScope(std::string_view name, ScopeID scope) const;
    bool containsInScope(const Identifier& name, ScopeID scope) const;

    /// Lookup within a single scope (no chain walk).
    /// Returns nullopt if not found in that specific scope.
    std::optional<SymbolID> lookupInScope(std::string_view name, ScopeID scope) const;
    std::optional<SymbolID> lookupInScope(const Identifier& name, ScopeID scope) const;

    // ── Direct Access by ID ───────────────────────────────────────────────────

    const Symbol& getSymbol(SymbolID id) const;
    Symbol&       getMutableSymbol(SymbolID id);

    const Scope&  getScope(ScopeID id) const;

    // ── Statistics ────────────────────────────────────────────────────────────

    size_t symbolCount() const { return arena_.size(); }
    size_t scopeCount()  const { return scopes_.size(); }

    /// Convenience: the global scope (always ScopeID 0).
    ScopeID globalScopeId() const { return 0; }

private:
    /// Arena of all symbols — indexed by SymbolID.
    /// Grows monotonically; IDs are stable indices.
    std::vector<Symbol> arena_;

    /// All scopes ever created — indexed by ScopeID.
    /// Includes the global scope at index 0.
    std::vector<Scope>  scopes_;
};

} // namespace fl
