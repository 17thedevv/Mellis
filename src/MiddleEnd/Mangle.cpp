#include "mellis/MiddleEnd/Mangle.h"
#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/SymbolTable.h"

namespace fl {
namespace Mangle {

static void typeToStringImpl(const Type* type, const SymbolTable& symTable, std::string& out) {
    if (!type) {
        out += "unknown";
        return;
    }

    switch (type->getKind()) {
        case TypeKind::Primitive: {
            out += dynamic_cast<const PrimitiveType*>(type)->toString();
            break;
        }
        case TypeKind::Struct: {
            auto* st = dynamic_cast<const StructType*>(type);
            const Symbol& sym = symTable.getSymbol(st->structSymbolId);
            out += sym.name.view();
            for (const auto* arg : st->genericArgs) {
                out += "_";
                typeToStringImpl(arg, symTable, out);
            }
            break;
        }
        case TypeKind::Enum: {
            auto* et = dynamic_cast<const EnumType*>(type);
            const Symbol& sym = symTable.getSymbol(et->enumSymbolId);
            out += sym.name.view();
            for (const auto* arg : et->genericArgs) {
                out += "_";
                typeToStringImpl(arg, symTable, out);
            }
            break;
        }
        case TypeKind::Pointer: {
            auto* pt = dynamic_cast<const PointerType*>(type);
            out += pt->isMutable ? "ptr_mut_" : "ptr_const_";
            typeToStringImpl(pt->pointee, symTable, out);
            break;
        }
        case TypeKind::Reference: {
            auto* rt = dynamic_cast<const ReferenceType*>(type);
            out += rt->isMutable ? "ref_mut_" : "ref_const_";
            typeToStringImpl(rt->pointee, symTable, out);
            break;
        }
        case TypeKind::Array: {
            auto* at = dynamic_cast<const ArrayType*>(type);
            out += "arr_";
            typeToStringImpl(at->elementType, symTable, out);
            break;
        }
        case TypeKind::Slice: {
            auto* st = dynamic_cast<const SliceType*>(type);
            out += "slice_";
            typeToStringImpl(st->elementType, symTable, out);
            break;
        }
        case TypeKind::Tuple: {
            auto* tt = dynamic_cast<const TupleType*>(type);
            out += "tuple";
            for (const auto* elem : tt->elements) {
                out += "_";
                typeToStringImpl(elem, symTable, out);
            }
            break;
        }
        case TypeKind::Function: {
            auto* ft = dynamic_cast<const FunctionType*>(type);
            out += "fn";
            for (const auto* param : ft->paramTypes) {
                out += "_";
                typeToStringImpl(param, symTable, out);
            }
            out += "_ret_";
            typeToStringImpl(ft->returnType, symTable, out);
            break;
        }
        case TypeKind::GenericParam: {
            auto* gp = dynamic_cast<const GenericParamType*>(type);
            out += gp->name;
            break;
        }
        case TypeKind::Void: {
            out += "void";
            break;
        }
        case TypeKind::Never: {
            out += "never";
            break;
        }
        default:
            out += "unmangled";
            break;
    }
}

std::string typeToString(const Type* type, const SymbolTable& symTable) {
    std::string out;
    out.reserve(64);
    typeToStringImpl(type, symTable, out);
    return out;
}

std::string mangleFunction(std::string_view baseName, const std::vector<const Type*>& genericArgs, const SymbolTable& symTable) {
    std::string out(baseName);
    for (const auto* arg : genericArgs) {
        out += "_";
        out += typeToString(arg, symTable);
    }
    return out;
}

std::string mangleStruct(std::string_view baseName, const std::vector<const Type*>& genericArgs, const SymbolTable& symTable) {
    std::string out(baseName);
    for (const auto* arg : genericArgs) {
        out += "_";
        out += typeToString(arg, symTable);
    }
    return out;
}

} // namespace Mangle
} // namespace fl
