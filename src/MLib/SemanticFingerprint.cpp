// =============================================================================
// mellis/MLib/SemanticFingerprint.cpp
// =============================================================================

#include "mellis/MLib/SemanticFingerprint.h"
#include <algorithm>
#include <vector>
#include <iomanip>
#include <sstream>

namespace fl {
namespace mlib {

// FNV-1a 64-bit hash function
static uint64_t hash_fnv1a(const std::string& text) {
    uint64_t hash = 0xcbf29ce484222325ull;
    for (char c : text) {
        hash ^= static_cast<uint8_t>(c);
        hash *= 0x100000001b3ull;
    }
    return hash;
}

static void canonicalizeType(const Type* type, std::ostringstream& out, const SemanticSnapshot& snapshot) {
    if (!type) {
        out << "N";
        return;
    }
    out << static_cast<int>(type->getKind()) << ":";
    
    switch (type->getKind()) {
        case TypeKind::Primitive: {
            auto prim = static_cast<const PrimitiveType*>(type);
            out << static_cast<int>(prim->builtinKind) << ";";
            break;
        }
        case TypeKind::Reference: {
            auto ref = static_cast<const ReferenceType*>(type);
            out << (ref->isMutable ? "M" : "I");
            canonicalizeType(ref->pointee, out, snapshot);
            break;
        }
        case TypeKind::Pointer: {
            auto ptr = static_cast<const PointerType*>(type);
            out << (ptr->isMutable ? "M" : "I");
            canonicalizeType(ptr->pointee, out, snapshot);
            break;
        }
        case TypeKind::Array: {
            auto arr = static_cast<const ArrayType*>(type);
            out << arr->length << ";";
            canonicalizeType(arr->elementType, out, snapshot);
            break;
        }
        case TypeKind::Slice: {
            auto slice = static_cast<const SliceType*>(type);
            canonicalizeType(slice->elementType, out, snapshot);
            break;
        }
        case TypeKind::Tuple: {
            auto tup = static_cast<const TupleType*>(type);
            out << tup->elements.size() << ";";
            for (auto* elem : tup->elements) {
                canonicalizeType(elem, out, snapshot);
            }
            break;
        }
        case TypeKind::Struct: {
            auto st = static_cast<const StructType*>(type);
            const Symbol& sym = snapshot.getSymbolTable().getSymbol(st->structSymbolId);
            out << sym.name.view() << ";";
            for (auto* arg : st->genericArgs) canonicalizeType(arg, out, snapshot);
            break;
        }
        case TypeKind::Enum: {
            auto en = static_cast<const EnumType*>(type);
            const Symbol& sym = snapshot.getSymbolTable().getSymbol(en->enumSymbolId);
            out << sym.name.view() << ";";
            for (auto* arg : en->genericArgs) canonicalizeType(arg, out, snapshot);
            break;
        }
        case TypeKind::Trait: {
            auto tr = static_cast<const TraitType*>(type);
            const Symbol& sym = snapshot.getSymbolTable().getSymbol(tr->traitId);
            out << sym.name.view() << ";";
            // Map keys should be sorted for determinism, but for now we just skip assoc bindings 
            // since they are usually canonicalized by the generic constraint list.
            break;
        }
        case TypeKind::TraitObject: {
            auto tro = static_cast<const TraitObjectType*>(type);
            const Symbol& sym = snapshot.getSymbolTable().getSymbol(tro->traitId);
            out << sym.name.view() << ";";
            break;
        }
        case TypeKind::Function: {
            auto fn = static_cast<const FunctionType*>(type);
            out << fn->paramTypes.size() << ";";
            for (auto* pt : fn->paramTypes) canonicalizeType(pt, out, snapshot);
            canonicalizeType(fn->returnType, out, snapshot);
            break;
        }
        case TypeKind::Lifetime: {
            auto lt = static_cast<const LifetimeType*>(type);
            out << static_cast<int>(lt->lt.kind) << ":" << lt->lt.name << ";";
            break;
        }
        default: {
            out << "?;";
            break;
        }
    }
}

uint64_t SemanticFingerprint::computeCanonicalHash(const Symbol& sym, const SemanticSnapshot& snapshot) {
    std::ostringstream signature;
    signature << static_cast<int>(sym.kind) << ":" << sym.name.view() << ";";
    const Type* t = snapshot.typeOf(sym.id);
    canonicalizeType(t, signature, snapshot);
    return hash_fnv1a(signature.str());
}

std::string SemanticFingerprint::compute(const SemanticSnapshot& snapshot) {
    std::vector<const Symbol*> exports;
    
    // Collect all exported symbols
    const auto& symTab = snapshot.getSymbolTable();
    for (uint32_t i = 0; i < symTab.symbolCount(); ++i) {
        const auto& sym = symTab.getSymbol(i);
        if (sym.visibility == Visibility::Public && !sym.isExternal) {
            exports.push_back(&sym);
        }
    }

    // Sort to guarantee determinism
    std::sort(exports.begin(), exports.end(), [](const Symbol* a, const Symbol* b) {
        return a->name.view() < b->name.view();
    });

    std::ostringstream signature;
    for (const Symbol* symPtr : exports) {
        const auto& sym = *symPtr;
        signature << static_cast<int>(sym.kind) << ":" << sym.name.view() << ";";
        
        // Deep semantic hashing
        const Type* t = snapshot.typeOf(sym.id);
        canonicalizeType(t, signature, snapshot);
    }

    uint64_t hash = hash_fnv1a(signature.str());
    
    std::ostringstream hexStream;
    hexStream << std::hex << std::setfill('0') << std::setw(16) << hash;
    return hexStream.str();
}

} // namespace mlib
} // namespace fl
