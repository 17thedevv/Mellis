// =============================================================================
// fdlang/MiddleEnd/TypeChecker.h
//
// TypeChecker — Evaluates and infers Semantic Types.
// =============================================================================
#pragma once
#include "fdlang/FrontEnd/ASTVisitor.h"
#include "fdlang/Core/FLType.h"
#include "fdlang/MiddleEnd/SymbolTable.h"
#include "fdlang/Support/Diagnostic.h"
#include <vector>

namespace fl {

class TypeChecker {
public:
    explicit TypeChecker(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx);
    bool check(ASTNode* root);
    const Type* typeOf(SymbolID id) const;

private:
    SymbolTable& table_;
    DiagnosticEngine& diag_;
    TypeContext& ctx_;
    std::vector<const Type*> typeTable_;
};

} // namespace fl
