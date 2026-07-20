#include "mellis/MLib/MetadataBuilder.h"

namespace fl {
namespace mlib {

MetadataBuilder::MetadataBuilder(StringTableBuilder& stringTable)
    : stringTable(stringTable) {}

uint32_t MetadataBuilder::addNamespace(const std::string& name, uint32_t parentNamespaceID) {
    uint32_t id = static_cast<uint32_t>(namespaces.size());
    NamespaceEntry entry;
    entry.nameStringID = stringTable.addString(name);
    entry.parentNamespaceID = parentNamespaceID;
    namespaces.push_back(entry);
    return id;
}

uint32_t MetadataBuilder::addType(const std::string& name, uint32_t namespaceID, uint64_t size, uint64_t alignment) {
    uint32_t id = static_cast<uint32_t>(types.size());
    TypeEntry entry;
    entry.nameStringID = stringTable.addString(name);
    entry.namespaceID = namespaceID;
    entry.size = size;
    entry.alignment = alignment;
    types.push_back(entry);
    return id;
}

uint32_t MetadataBuilder::addTrait(const std::string& name, uint32_t namespaceID) {
    uint32_t id = static_cast<uint32_t>(traits.size());
    TraitEntry entry;
    entry.nameStringID = stringTable.addString(name);
    entry.namespaceID = namespaceID;
    traits.push_back(entry);
    return id;
}

uint32_t MetadataBuilder::addFunction(const std::string& name, uint32_t namespaceID, uint32_t signatureTypeID) {
    uint32_t id = static_cast<uint32_t>(functions.size());
    FunctionEntry entry;
    entry.nameStringID = stringTable.addString(name);
    entry.namespaceID = namespaceID;
    entry.signatureTypeID = signatureTypeID;
    functions.push_back(entry);
    return id;
}

uint32_t MetadataBuilder::addImpl(uint32_t traitID, uint32_t targetTypeID) {
    uint32_t id = static_cast<uint32_t>(impls.size());
    ImplEntry entry;
    entry.traitID = traitID;
    entry.targetTypeID = targetTypeID;
    impls.push_back(entry);
    return id;
}

void MetadataBuilder::serializeMetadata(BinaryWriter& writer) const {
    // Structure of Metadata Section:
    // uint32_t namespaceCount; [NamespaceEntry...]
    // uint32_t typeCount; [TypeEntry...]
    // uint32_t traitCount; [TraitEntry...]
    // uint32_t functionCount; [FunctionEntry...]
    
    writer.writeU32(static_cast<uint32_t>(namespaces.size()));
    for (const auto& entry : namespaces) writer.writeStruct(entry);

    writer.writeU32(static_cast<uint32_t>(types.size()));
    for (const auto& entry : types) writer.writeStruct(entry);

    writer.writeU32(static_cast<uint32_t>(traits.size()));
    for (const auto& entry : traits) writer.writeStruct(entry);

    writer.writeU32(static_cast<uint32_t>(functions.size()));
    for (const auto& entry : functions) writer.writeStruct(entry);
}

void MetadataBuilder::serializeImpls(BinaryWriter& writer) const {
    writer.writeU32(static_cast<uint32_t>(impls.size()));
    for (const auto& entry : impls) writer.writeStruct(entry);
}

} // namespace mlib
} // namespace fl
