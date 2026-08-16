#include "mellis/MiddleEnd/VisibilityLeakChecker.h"
#include "mellis/MiddleEnd/TypeChecker.h"

namespace fl {

VisibilityLeakChecker::VisibilityLeakChecker(const SymbolTable& symTab, const TypeChecker& checker, DiagnosticEngine& diag)
    : symTab_(symTab), checker_(checker), diag_(diag) {}

bool VisibilityLeakChecker::check() {
    bool ok = true;
    for (uint32_t i = 0; i < symTab_.symbolCount(); ++i) {
        const auto& sym = symTab_.getSymbol(i);
        if (sym.visibility != Visibility::Public) continue;

        if (sym.kind == SymbolKind::Function) {
            if (const Type* t = checker_.typeOf(sym.id)) {
                if (t->getKind() == TypeKind::Function) {
                    const auto* fnType = static_cast<const FunctionType*>(t);
                    for (const auto* paramT : fnType->paramTypes) {
                        if (!checkType(paramT, sym, "function parameter")) ok = false;
                    }
                    if (!checkType(fnType->returnType, sym, "function return type")) ok = false;
                }
            }
        } else if (sym.kind == SymbolKind::Struct) {
            if (const Type* t = checker_.typeOf(sym.id)) {
                if (t->getKind() == TypeKind::Struct) {
                    const auto* structType = static_cast<const StructType*>(t);
                    for (const auto* paramT : structType->fieldTypes) {
                        if (!checkType(paramT, sym, "struct field")) ok = false;
                    }
                }
            }
        } else if (sym.kind == SymbolKind::Trait) {
            if (const Type* t = checker_.typeOf(sym.id)) {
                if (t->getKind() == TypeKind::Trait) {
                    // Methods of a trait might not be directly available in TraitType without Methods.
                    // But the methods are in the SymbolTable under this scope! Wait, trait methods are stored in a scope?
                    // We can just iterate over all functions and if their declaredInScope is the trait's scope, they are trait methods!
                }
            }
        }
    }
    return ok;
}

bool VisibilityLeakChecker::checkType(const Type* t, const Symbol& ownerSym, const std::string& context) {
    std::string privateName;
    if (isPrivateType(t, privateName)) {
        diag_.error(ownerSym.location, "public " + std::string(ownerSym.name.view()) + " leaks private type '" + privateName + "' in " + context);
        return false;
    }
    return true;
}

bool VisibilityLeakChecker::isPrivateType(const Type* t, std::string& outPrivateName) const {
    if (!t) return false;
    switch (t->getKind()) {
        case TypeKind::Struct: {
            SymbolID id = static_cast<const StructType*>(t)->structSymbolId;
            const auto& sym = symTab_.getSymbol(id);
            if (sym.visibility != Visibility::Public) {
                outPrivateName = std::string(sym.name.view());
                return true;
            }
            break;
        }
        case TypeKind::Enum: {
            SymbolID id = static_cast<const EnumType*>(t)->enumSymbolId;
            const auto& sym = symTab_.getSymbol(id);
            if (sym.visibility != Visibility::Public) {
                outPrivateName = std::string(sym.name.view());
                return true;
            }
            break;
        }
        case TypeKind::Trait: {
            SymbolID id = static_cast<const TraitType*>(t)->traitId;
            const auto& sym = symTab_.getSymbol(id);
            if (sym.visibility != Visibility::Public) {
                outPrivateName = std::string(sym.name.view());
                return true;
            }
            break;
        }
        case TypeKind::Pointer: return isPrivateType(static_cast<const PointerType*>(t)->pointee, outPrivateName);
        case TypeKind::Reference: return isPrivateType(static_cast<const ReferenceType*>(t)->pointee, outPrivateName);
        case TypeKind::Array: return isPrivateType(static_cast<const ArrayType*>(t)->elementType, outPrivateName);
        case TypeKind::Slice: return isPrivateType(static_cast<const SliceType*>(t)->elementType, outPrivateName);
        case TypeKind::Function: {
            const auto* fnType = static_cast<const FunctionType*>(t);
            for (const auto* p : fnType->paramTypes) {
                if (isPrivateType(p, outPrivateName)) return true;
            }
            return isPrivateType(fnType->returnType, outPrivateName);
        }
        case TypeKind::Tuple: {
            const auto* tup = static_cast<const TupleType*>(t);
            for (const auto* e : tup->elements) {
                if (isPrivateType(e, outPrivateName)) return true;
            }
            break;
        }
        default: break;
    }
    return false;
}

} // namespace fl
