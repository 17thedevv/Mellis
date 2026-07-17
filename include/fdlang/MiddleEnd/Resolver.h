// =============================================================================
// fdlang/MiddleEnd/Resolver.h
//
// Resolver — the Name Resolution pass (Multi-Pass Architecture).
//
// Single responsibility:
//   Connect every identifier USE to its DECLARATION by annotating AST nodes
//   with SymbolIDs.
//
// Multi-Pass Architecture:
//   Pass 0: Register Builtins into the Global Scope.
//   Pass 1: Register Declarations (Structs, Functions, Traits, Variables, etc.)
//           Does not resolve expressions.
//   Pass 2: Resolve Usages (Function bodies, expressions, type annotations).
//           Maps IdentifierExpr and NamedTypeNode to SymbolIDs.
// =============================================================================

#pragma once
#include "fdlang/MiddleEnd/SymbolTable.h"
#include "fdlang/Support/Diagnostic.h"

namespace fl {

class ProgramNode;

class Resolver {
public:
    /// @param table  The SymbolTable to populate. Must outlive this Resolver.
    /// @param diag   The DiagnosticEngine to receive all error/warning reports.
    explicit Resolver(SymbolTable& table, DiagnosticEngine& diag);

    /// Performs symbol resolution on the given AST root.
    /// Returns true if resolution succeeded without errors.
    bool resolve(ASTNode* root);

    /// Number of resolution errors.
    size_t errorCount() const { return diag_.errorCount(); }

private:
    SymbolTable&      table_;
    DiagnosticEngine& diag_;
};

} // namespace fl
