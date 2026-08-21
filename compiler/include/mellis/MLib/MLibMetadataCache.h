#ifndef MELLIS_MLIB_MLIBMETADATACACHE_H
#define MELLIS_MLIB_MLIBMETADATACACHE_H

// =============================================================================
// mellis/MLib/MLibMetadataCache.h
//
// MLibMetadataCache — the TypeChecker-facing bridge to external .mlib types.
//
// Responsibility:
//   Convert binary ExternalSymbol metadata (mlibSymbolID + moduleUUID) into
//   live FLType* objects that the TypeChecker can use directly.
//
// Design:
//   - Owns NO binary data. All binary data lives in ModuleLoader / memory maps.
//   - Receives pre-decoded TypeSignature entries from ModuleLoader at load time.
//   - Answers O(1) lookups: getType(symbolID) -> const Type*
//   - All returned Type* point into the TypeContext arena (stable lifetime).
//
// Phase boundary: this is a MiddleEnd component that consumes MLib data.
// It must not write to .mlib, only read.
// =============================================================================

#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/Symbol.h"
#include <unordered_map>
#include <cstdint>
#include <cstring>

namespace fl {

class MLibMetadataCache {
public:
    explicit MLibMetadataCache(TypeContext& ctx);

    // Register the type for an ExternalSymbol.
    // Called by ModuleLoader after parsing a FunctionEntry's signature.
    // symbolID is the SymbolID assigned by declareExternalSymbol().
    void registerType(SymbolID symbolID, const Type* type);

    // Query the type for an ExternalSymbol.
    // Returns ctx.getUnknown() if not registered.
    const Type* getType(SymbolID symbolID) const;

    // Convenience: build a FunctionType from primitive descriptors.
    // Used by ModuleLoader to pre-construct types before registration.
    // paramPrimitives: array of BuiltinKind values for each parameter.
    // retPrimitive:    BuiltinKind of the return type.
    const FunctionType* buildExternalFunctionType(
        const std::vector<std::string>& paramNames,
        const std::vector<BuiltinKind>& paramKinds,
        BuiltinKind retKind,
        bool isVariadic = false);

    // Overload: return type is opaque (external struct).
    const FunctionType* buildExternalFunctionTypeOpaque(
        const std::vector<std::string>& paramNames,
        const std::vector<BuiltinKind>& paramKinds,
        const Type* retType,
        bool isVariadic = false);

private:
    TypeContext& ctx;
    std::unordered_map<SymbolID, const Type*> cache;
};

} // namespace fl

#endif // MELLIS_MLIB_MLIBMETADATACACHE_H
