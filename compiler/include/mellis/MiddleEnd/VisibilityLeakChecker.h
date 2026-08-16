// =============================================================================
// mellis/MiddleEnd/VisibilityLeakChecker.h
//
// Enforces deep visibility leakage rules:
// - A public struct cannot use a private type in a public field.
// - A public function cannot take or return a private type.
// - A public trait cannot use a private type.
// =============================================================================
#pragma once

#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/Core/FLType.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class TypeChecker;

class VisibilityLeakChecker {
public:
    VisibilityLeakChecker(const SymbolTable& symTab, const TypeChecker& checker, DiagnosticEngine& diag);
    
    // Checks all symbols. Returns true if no leaks found.
    bool check();

private:
    const SymbolTable& symTab_;
    const TypeChecker& checker_;
    DiagnosticEngine& diag_;

    bool checkType(const Type* t, const Symbol& ownerSym, const std::string& context);
    bool isPrivateType(const Type* t, std::string& outPrivateName) const;
};

} // namespace fl
