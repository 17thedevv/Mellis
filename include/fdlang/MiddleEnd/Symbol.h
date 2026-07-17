// =============================================================================
// fdlang/MiddleEnd/Symbol.h
//
// Core data structures for the name resolution layer:
//   SymbolKind  — what kind of entity a Symbol represents
//   ScopeKind   — what kind of lexical context a Scope represents
//   Symbol      — one resolved name (lives in SymbolTable::arena_)
//   Scope       — one lexical scope (lives in SymbolTable::scopes_)
//   ScopeBindings — the name→ID map inside each Scope
//
// Design philosophy:
//   Symbol is intentionally MINIMAL. It stores ONLY what the Resolver
//   needs to produce and what downstream passes need to READ:
//     ✓ id, scope, name, kind, location, decl pointer
//     ✗ type info         → TypeChecker's responsibility
//     ✗ borrow state      → BorrowChecker's responsibility
//     ✗ llvm::Value*      → IRGenerator's responsibility
//     ✗ isInitialized     → SemanticAnalyzer / TypeChecker
//
//   This keeps the Symbol small and cache-friendly. Future passes add
//   their own side tables (e.g., TypeTable[SymbolID] → Type).
// =============================================================================

#pragma once
#include "fdlang/Core/Types.h"
#include "fdlang/Core/Identifier.h"
#include "fdlang/Core/SourceLocation.h"
#include "fdlang/AST/ASTNode.h"           // ASTNode* for decl back-pointer
#include <unordered_map>
#include <cstdint>

namespace fl {

// =============================================================================
// SymbolKind — what a Symbol represents
// =============================================================================
enum class SymbolKind : uint8_t {
    Variable,     // dec x
    Const,        // const x
    Parameter,    // fn foo(x: i32)
    Function,     // fn foo()
    Struct,       // struct Foo
    Enum,         // enum Bar
    EnumVariant,  // Bar::Baz
    StructField,  // Foo.x
    Trait,        // trait T
    TraitMethod,  // trait T { fn foo(); }
    Impl,         // impl T for U
    TypeAlias,    // type X = Y;
    GenericParam, // <T>
    Module,       // mod m
    Namespace,    // Logical grouping
};

// =============================================================================
// ScopeKind — what lexical context a Scope was created in
//
// Used by future passes to understand the semantic meaning of a scope:
//   - BorrowChecker: variables in Loop scopes need drop-at-end-of-iteration
//   - TypeChecker: Function scopes introduce a return type context
//   - Diagnostics: "variable declared inside 'while' body" error messages
// =============================================================================
enum class ScopeKind : uint8_t {
    Global,       // top-level program scope
    Module,       // mod { } (future)
    Function,     // fn() { } (future)
    Block,        // { } — generic block
    Loop,         // while / for body (future: ScopeKind::Loop for break/continue)
    Conditional,  // if / else branch
    Struct,       // struct body
    Enum,         // enum body
    Trait,        // trait body
    Impl,         // impl body
};

// =============================================================================
// ScopeBindings — the name→SymbolID map inside a single Scope.
//
// Uses Identifier with transparent hash/equal so lookup can accept
// raw string_view without constructing an Identifier object:
//   bindings.find("x")           — O(1), zero allocation
//   bindings.find(some_sv)       — O(1), zero allocation
// =============================================================================
using ScopeBindings = std::unordered_map<Identifier, SymbolID,
                                         IdentifierHash, IdentifierEqual>;

// =============================================================================
// Symbol — one entry in the SymbolTable arena.
//
// Arena layout: arena_[symbolId] == Symbol with symbol.id == symbolId.
// IDs are stable: the vector never reallocates mid-compilation
// because we reserve() upfront (handled by SymbolTable constructor).
//
// The `decl` back-pointer enables downstream passes to navigate from a
// resolved reference back to the original AST declaration node:
//   TypeChecker:  reads the type annotation on the VarDeclStmt
//   BorrowChecker: checks ownership state at the declaration site
//   Diagnostics:  prints "originally declared at line X"
//   IRGenerator:  emits alloca at the function entry for each VarDeclStmt
// =============================================================================
struct Symbol {
    SymbolID       id;                // index into SymbolTable::arena_
    ScopeID        declaredInScope;   // which scope this was declared in
    Identifier     name;             // internable identifier (owns the string in MVP)
    SymbolKind     kind;
    SourceLocation location;         // declaration site — for diagnostics
    ASTNode*       decl = nullptr;   // back-pointer to declaring AST node (non-owning)
                                     // e.g. VarDeclStmt*, FuncDecl* (future)
};

// =============================================================================
// Scope — one lexical scope in the scope tree.
//
// Scope tree (not just a stack) because ALL scopes persist after Resolver:
//   TypeChecker queries "what scope was this symbol declared in?"
//   BorrowChecker walks the scope tree to compute variable lifetimes.
//   Diagnostics reconstructs the full scope path for error messages.
// =============================================================================
struct Scope {
    ScopeID              id;
    ScopeID              parentId;   // kInvalidScopeID for the global scope
    std::vector<ScopeID> children;   // Tree hierarchy for scoping
    ScopeKind            kind;
    ScopeBindings        bindings;   // name → SymbolID within this scope only
};

} // namespace fl
