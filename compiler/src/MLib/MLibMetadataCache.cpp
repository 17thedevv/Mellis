#include "mellis/MLib/MLibMetadataCache.h"

namespace fl {

MLibMetadataCache::MLibMetadataCache(TypeContext& ctx) : ctx(ctx) {}

void MLibMetadataCache::registerType(SymbolID symbolID, const Type* type) {
    cache[symbolID] = type;
}

const Type* MLibMetadataCache::getType(SymbolID symbolID) const {
    auto it = cache.find(symbolID);
    return (it != cache.end()) ? it->second : ctx.getUnknown();
}

const FunctionType* MLibMetadataCache::buildExternalFunctionType(
        const std::vector<std::string>& paramNames,
        const std::vector<BuiltinKind>& paramKinds,
        BuiltinKind retKind,
        bool isVariadic) {

    std::vector<const Type*> paramTypes;
    paramTypes.reserve(paramKinds.size());
    for (auto k : paramKinds) {
        paramTypes.push_back(ctx.getPrimitive(k));
    }

    const Type* retType = ctx.getPrimitive(retKind);
    return ctx.create<FunctionType>(
        std::vector<std::string>(paramNames),
        std::move(paramTypes),
        retType,
        false,      // callSite = false (this is a declaration)
        isVariadic
    );
}

const FunctionType* MLibMetadataCache::buildExternalFunctionTypeOpaque(
        const std::vector<std::string>& paramNames,
        const std::vector<BuiltinKind>& paramKinds,
        const Type* retType,
        bool isVariadic) {

    std::vector<const Type*> paramTypes;
    paramTypes.reserve(paramKinds.size());
    for (auto k : paramKinds) {
        paramTypes.push_back(ctx.getPrimitive(k));
    }

    return ctx.create<FunctionType>(
        std::vector<std::string>(paramNames),
        std::move(paramTypes),
        retType,
        false,
        isVariadic
    );
}

} // namespace fl
