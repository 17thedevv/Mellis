#include "fdlang/MiddleEnd/SymbolTable.h"
#include <cassert>

namespace fl {

// =============================================================================
// Construction
// =============================================================================

SymbolTable::SymbolTable() {
    // Pre-allocate to avoid rehashing during typical compilation.
    // 256 symbols and 32 scopes covers most single-file programs.
    arena_.reserve(256);
    scopes_.reserve(32);

    // Eagerly create the global scope (ScopeID = 0).
    // parentId = kInvalidScopeID signals "no enclosing scope".
    scopes_.push_back(Scope{
        /* id       = */ 0,
        /* parentId = */ kInvalidScopeID,
        /* children = */ {},
        /* kind     = */ ScopeKind::Global,
        /* bindings = */ ScopeBindings{}
    });
}

// =============================================================================
// Scope Management
// =============================================================================

ScopeID SymbolTable::createScope(ScopeKind kind, ScopeID parentId) {
    // Validate parentId (unless it's the sentinel for global scope).
    assert((parentId == kInvalidScopeID || parentId < scopes_.size()) &&
           "createScope: invalid parentId");

    ScopeID newId = static_cast<ScopeID>(scopes_.size());
    scopes_.push_back(Scope{newId, parentId, {}, kind, ScopeBindings{}});
    
    if (parentId != kInvalidScopeID) {
        scopes_[parentId].children.push_back(newId);
    }
    
    return newId;
}

// =============================================================================
// Symbol Declaration
// =============================================================================

SymbolID SymbolTable::declareSymbol(const Identifier& name,
                                     SymbolKind        kind,
                                     ScopeID           scope,
                                     SourceLocation    location,
                                     ASTNode*          decl) {
    assert(scope < scopes_.size() && "declareSymbol: invalid ScopeID");

    SymbolID newId = static_cast<SymbolID>(arena_.size());

    arena_.push_back(Symbol{
        /* id              = */ newId,
        /* declaredInScope = */ scope,
        /* name            = */ name,
        /* kind            = */ kind,
        /* location        = */ location,
        /* decl            = */ decl
    });

    // Register this name in the scope's binding table.
    // Precondition: caller verified no duplicate via containsInScope().
    scopes_[scope].bindings.emplace(name, newId);

    return newId;
}

// =============================================================================
// Lookup: Chain Walk
// =============================================================================

std::optional<SymbolID> SymbolTable::lookup(std::string_view name,
                                             ScopeID fromScope) const {
    ScopeID current = fromScope;

    while (current != kInvalidScopeID) {
        assert(current < scopes_.size() && "lookup: corrupted scope chain");
        const Scope& scope = scopes_[current];

        // MSVC C++17: unordered_map transparent lookup requires C++20.
        // Construct Identifier explicitly for the find() call.
        // Cost: one std::string construction per scope level per lookup.
        // Future: upgrade to C++20 or use a custom hash map with C++17 find(string_view).
        Identifier key(name);
        auto it = scope.bindings.find(key);
        if (it != scope.bindings.end()) {
            return it->second;
        }

        current = scope.parentId;
    }

    return std::nullopt;
}

std::optional<SymbolID> SymbolTable::lookup(const Identifier& name,
                                             ScopeID fromScope) const {
    return lookup(name.view(), fromScope);
}

// =============================================================================
// Lookup: Single Scope
// =============================================================================

bool SymbolTable::containsInScope(std::string_view name, ScopeID scope) const {
    assert(scope < scopes_.size() && "containsInScope: invalid ScopeID");
    Identifier key(name);
    return scopes_[scope].bindings.count(key) > 0;
}

bool SymbolTable::containsInScope(const Identifier& name, ScopeID scope) const {
    return containsInScope(name.view(), scope);
}

std::optional<SymbolID> SymbolTable::lookupInScope(std::string_view name,
                                                    ScopeID scope) const {
    assert(scope < scopes_.size() && "lookupInScope: invalid ScopeID");
    Identifier key(name);
    auto it = scopes_[scope].bindings.find(key);
    if (it != scopes_[scope].bindings.end()) {
        return it->second;
    }
    return std::nullopt;
}

std::optional<SymbolID> SymbolTable::lookupInScope(const Identifier& name,
                                                    ScopeID scope) const {
    return lookupInScope(name.view(), scope);
}

// =============================================================================
// Direct Access by ID
// =============================================================================

const Symbol& SymbolTable::getSymbol(SymbolID id) const {
    assert(id < arena_.size() && "getSymbol: invalid SymbolID");
    return arena_[id];
}

Symbol& SymbolTable::getMutableSymbol(SymbolID id) {
    assert(id < arena_.size() && "getMutableSymbol: invalid SymbolID");
    return arena_[id];
}

const Scope& SymbolTable::getScope(ScopeID id) const {
    assert(id < scopes_.size() && "getScope: invalid ScopeID");
    return scopes_[id];
}

} // namespace fl
