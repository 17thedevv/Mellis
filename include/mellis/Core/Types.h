// =============================================================================
// mellis/Core/Types.h
//
// Primitive ID types shared across all compiler passes.
//
// Design rationale:
//   - uint32_t is the standard choice in production compilers (rustc's DefId,
//     LLVM's SlotIndex). 4 billion symbols per file is effectively unlimited.
//   - Sentinel values (UINT32_MAX) are used instead of std::optional<ID> in
//     hot paths to avoid the 8-byte overhead per node (alignment padding).
//   - IDs are opaque — consumers must not do arithmetic on them.
//
// Future: If multi-file / multi-crate is needed, SymbolID can be promoted
// to struct { FileID file; uint32_t local_id; } with zero API breakage
// because all callers use the typedef.
// =============================================================================

#pragma once
#include <cstdint>
#include <limits>

namespace fl {

// ── Symbol Identification ─────────────────────────────────────────────────────

/// Unique identifier for a declared symbol (variable, function, struct, …).
/// Indexes into SymbolTable::arena_. O(1) access guaranteed.
using SymbolID = uint32_t;
using MacroID = uint32_t;
using ModuleID = uint32_t;
using ExpansionID = uint32_t;

/// Sentinel: "this node has not been resolved yet."
/// Set as default in every AST node that carries a SymbolID.
/// After Resolver runs, any remaining kInvalidSymbolID indicates a bug
/// (either undeclared name or a Resolver path that forgot to annotate).
constexpr SymbolID kInvalidSymbolID = std::numeric_limits<SymbolID>::max();
constexpr MacroID kInvalidMacroID = std::numeric_limits<MacroID>::max();
constexpr ModuleID kInvalidModuleID = std::numeric_limits<ModuleID>::max();

// ── Scope Identification ──────────────────────────────────────────────────────

/// Unique identifier for a lexical scope.
/// Indexes into SymbolTable::scopes_. O(1) access guaranteed.
using ScopeID = uint32_t;

/// Sentinel: "no parent scope" (used by the global scope).
constexpr ScopeID kInvalidScopeID = std::numeric_limits<ScopeID>::max();

// ── File Identification ───────────────────────────────────────────────────────

/// Identifies a source file. MVP: only one file, always kMainFileID.
/// Future: multi-file compilation, each file gets a unique FileID.
using FileID = uint32_t;

constexpr FileID kMainFileID = 0;

} // namespace fl
