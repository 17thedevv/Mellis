#include "mellis/MiddleEnd/Semantic/ObjectSafety.h"
#include "mellis/AST/DeclNode.h"

namespace fl {

bool ObjectSafety::isObjectSafe(SymbolID traitId, TypeContext& ctx, SymbolTable& table, const std::vector<const Type*>& typeTable, DiagnosticEngine& diag, SourceLocation loc) {
    const Symbol& sym = table.getSymbol(traitId);
    if (!sym.decl || sym.kind != SymbolKind::Trait) return false;

    auto* traitDecl = static_cast<const TraitDeclNode*>(sym.decl);
    bool isSafe = true;

    for (const auto& method : traitDecl->methods) {
        if (method->symbolId != kInvalidSymbolID && method->symbolId < typeTable.size()) {
            const Type* methodType = typeTable[method->symbolId];
            if (auto* fnType = dynamic_cast<const FunctionType*>(methodType)) {
                // Rule 1: A trait is not object safe if any of its methods have generic parameters.
                // In Mellis, we check the AST for generics since semantic FunctionType might not store generic definitions explicitly yet, 
                // but the proper way is semantic. For now, the combination of AST genericParams + semantic check is best.
                bool hasTypeOrConstGenerics = false;
                for (const auto& gp : method->genericParams) {
                    if (gp.kind != GenericParamKind::Lifetime) {
                        hasTypeOrConstGenerics = true;
                        break;
                    }
                }
                if (hasTypeOrConstGenerics) {
                    if (loc.line != 0) {
                        diag.error(loc, "E-OBJECT-SAFE: Trait '" + std::string(sym.name.view()) + 
                                        "' is not object-safe: method '" + std::string(method->name) + 
                                        "' has generic parameters.");
                    }
                    isSafe = false;
                }
                // Rule 2: Cannot return Self
                if (auto* genParam = dynamic_cast<const GenericParamType*>(fnType->returnType)) {
                    if (genParam->name == "Self") {
                        if (loc.line != 0) {
                            diag.error(loc, "E-OBJECT-SAFE: Trait '" + std::string(sym.name.view()) + 
                                            "' is not object-safe: method '" + std::string(method->name) + 
                                            "' returns `Self`.");
                        }
                        isSafe = false;
                    }
                }
                
                // Rule 3: Must have self, and self must be passed by reference
                bool hasSelf = false;
                if (!method->params.empty() && method->params[0]->isSelf) {
                    hasSelf = true;
                }
                
                if (!hasSelf) {
                    if (loc.line != 0) {
                        diag.error(loc, "E-OBJECT-SAFE: Trait '" + std::string(sym.name.view()) + 
                                        "' is not object-safe: method '" + std::string(method->name) + 
                                        "' is an associated function (no `self`).");
                    }
                    isSafe = false;
                } else {
                    auto* firstParam = fnType->paramTypes[0];
                    if (!dynamic_cast<const ReferenceType*>(firstParam) && !dynamic_cast<const PointerType*>(firstParam)) {
                        if (loc.line != 0) {
                            diag.error(loc, "E-OBJECT-SAFE: Trait '" + std::string(sym.name.view()) + 
                                            "' is not object-safe: method '" + std::string(method->name) + 
                                            "' passes `self` by value.");
                        }
                        isSafe = false;
                    }
                }
            } else {
                isSafe = false;
            }
        }
    }

    return isSafe;
}

}
