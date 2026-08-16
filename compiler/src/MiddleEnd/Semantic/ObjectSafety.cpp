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
                if (!method->genericParams.empty()) {
                    if (loc.line != 0) {
                        diag.error(loc, "E-OBJECT-SAFE: Trait '" + std::string(sym.name.view()) + 
                                        "' is not object-safe: method '" + std::string(method->name) + 
                                        "' has generic parameters.");
                    }
                    isSafe = false;
                }
                
                // Semantic check: does the method return Self?
                // The `Self` type is represented as a GenericParamType with paramId == traitId + something, or a specific placeholder.
                // For now, let's look for any generic param that matches the trait's Self.
            }
            isSafe = false;
        }

        // Check if `self` is passed by value.
        // In the AST, `ParamDeclNode::isSelf` is true for the receiver.
        // We need to look at its type. If it's not a ReferenceType (in Semantic Type) or PointerType, it's passed by value.
        // But since we are looking at AST here, how do we know?
        // Let's use `method->params[0]`. If `isSelf` is true, the `type` might be evaluated.
        // Alternatively, wait until it's lowered to semantic types, or check the AST.
        // In Mellis, `self` param usually has type `ReferenceTypeNode` or `PointerTypeNode` if it's passed by ref.
        if (!method->params.empty() && method->params[0]->isSelf) {
            auto* selfParam = method->params[0].get();
            // In Mellis parser, `&self` creates a ReferenceTypeNode.
            // If it's just `self`, it's an IdentifierTypeNode pointing to Self.
            // We consider it pass-by-value if it's not a reference/pointer.
            // Wait, we need the evaluated type! So `typeTable` or `inferredType` on the method's param?
            // Actually, we can check `selfParam->type->inferredType` if TypeChecker has already visited it.
        }
    }

    return isSafe;
}

}
