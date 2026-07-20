#ifndef MELLIS_MLIB_METADATABUILDER_H
#define MELLIS_MLIB_METADATABUILDER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/StringTableBuilder.h"
#include <vector>
#include <cstdint>
#include <string>

namespace fl {
namespace mlib {

class MetadataBuilder {
public:
    explicit MetadataBuilder(StringTableBuilder& stringTable);

    // Namespaces
    uint32_t addNamespace(const std::string& name, uint32_t parentNamespaceID = 0xFFFFFFFF);
    
    // Types
    uint32_t addType(const std::string& name, uint32_t namespaceID, uint64_t size, uint64_t alignment);
    
    // Traits
    uint32_t addTrait(const std::string& name, uint32_t namespaceID);
    
    // Functions
    uint32_t addFunction(const std::string& name, uint32_t namespaceID, uint32_t signatureTypeID);
    
    // Impls (These go into a separate section typically, but managed here for convenience)
    uint32_t addImpl(uint32_t traitID, uint32_t targetTypeID);

    // Serialize all metadata tables (Namespace, Type, Trait, Function)
    void serializeMetadata(BinaryWriter& writer) const;

    // Serialize impl table separately
    void serializeImpls(BinaryWriter& writer) const;

private:
    StringTableBuilder& stringTable;

    std::vector<NamespaceEntry> namespaces;
    std::vector<TypeEntry> types;
    std::vector<TraitEntry> traits;
    std::vector<FunctionEntry> functions;
    std::vector<ImplEntry> impls;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_METADATABUILDER_H
