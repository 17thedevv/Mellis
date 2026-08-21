#ifndef MELLIS_MLIB_METADATABUILDER_H
#define MELLIS_MLIB_METADATABUILDER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/StringTableBuilder.h"
#include "mellis/MiddleEnd/SemanticSnapshot.h"
#include <vector>
#include <cstdint>
#include <string>

namespace fl {
namespace mlib {

class MetadataBuilder {
public:
    explicit MetadataBuilder(StringTableBuilder& stringTable);

    void buildFromSnapshot(const SemanticSnapshot& snapshot);

    // Namespaces
    uint32_t addNamespace(const std::string& name, uint32_t parentNamespaceID = 0xFFFFFFFF);
    
    // Types
    uint32_t addType(const std::string& name, uint32_t namespaceID, uint64_t size, uint64_t alignment);
    
    // Traits
    uint32_t addTrait(const std::string& name, uint32_t namespaceID);
    
    // Functions
    uint32_t addFunction(const std::string& name, uint32_t namespaceID, uint32_t signatureTypeID);
    
    // Impls (These go into a separate section typically, but managed here for convenience)
    uint32_t addImpl(const ImplEntry& entry);

    // Serialize all metadata tables (Namespace, Type, Trait, Function)
    void serializeMetadata(BinaryWriter& writer) const;

    // Serialize impl table separately
    void serializeImpls(BinaryWriter& writer) const;

    // TypeRefs
    uint32_t addTypeRef(const fl::Type* type);
    void serializeTypeRefs(BinaryWriter& writer) const;

private:
    StringTableBuilder& stringTable;
    const SemanticSnapshot* snapshot_ = nullptr;

    struct InternalNamespace {
        std::string name;
        NamespaceEntry entry;
    };
    struct InternalType {
        std::string name;
        TypeEntry entry;
    };
    struct InternalTrait {
        std::string name;
        TraitEntry entry;
    };
    struct InternalFunction {
        std::string name;
        FunctionEntry entry;
    };

    struct InternalTypeRef {
        TypeRefRecord record;
        std::vector<uint32_t> payload; // For variable length data like args
    };

    std::vector<InternalNamespace> namespaces;
    std::vector<InternalType> types;
    std::vector<InternalTrait> traits;
    std::vector<InternalFunction> functions;
    std::vector<ImplEntry> impls;
    std::vector<InternalTypeRef> typeRefs;
    std::vector<std::vector<uint32_t>> implPayloads; // parallel to impls
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_METADATABUILDER_H
