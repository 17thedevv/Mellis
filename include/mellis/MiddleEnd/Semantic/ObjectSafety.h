#ifndef MELLIS_MIDDLEEND_OBJECTSAFETY_H
#define MELLIS_MIDDLEEND_OBJECTSAFETY_H

#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class ObjectSafety {
public:
    static bool isObjectSafe(SymbolID traitId, TypeContext& ctx, SymbolTable& table, const std::vector<const Type*>& typeTable, DiagnosticEngine& diag, SourceLocation loc = {});
};

}

#endif // MELLIS_MIDDLEEND_OBJECTSAFETY_H
