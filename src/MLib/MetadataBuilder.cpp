#include "mellis/MLib/MetadataBuilder.h"
#include <algorithm>

#include <unordered_set>

namespace fl {
namespace mlib {

MetadataBuilder::MetadataBuilder(StringTableBuilder& stringTable)
    : stringTable(stringTable) {}

void MetadataBuilder::buildFromSnapshot(const SemanticSnapshot& snapshot) {
    const auto& symTab = snapshot.getSymbolTable();
    std::vector<const Symbol*> exports;
    for (uint32_t i = 0; i < symTab.symbolCount(); ++i) {
        const auto& sym = symTab.getSymbol(i);
        if (sym.visibility == Visibility::Public && !sym.isExternal) {
            exports.push_back(&sym);
        }
    }
    std::sort(exports.begin(), exports.end(), [](const Symbol* a, const Symbol* b) {
        return a->name.view() < b->name.view();
    });

    std::unordered_set<const Symbol*> visited;
    
    auto visit = [&](auto& self, const Symbol* sym) -> void {
        if (!sym || visited.count(sym)) return;
        visited.insert(sym);

        if (sym->kind == SymbolKind::Struct) {
            addType(std::string(sym->name.view()), 0, 0, 0); // We will refine this
        } else if (sym->kind == SymbolKind::Function) {
            addFunction(std::string(sym->name.view()), 0, 0);
        } else if (sym->kind == SymbolKind::Trait) {
            addTrait(std::string(sym->name.view()), 0);
        } else if (sym->kind == SymbolKind::Enum) {
            addType(std::string(sym->name.view()), 0, 0, 0);
        } else if (sym->kind == SymbolKind::TypeAlias) {
            addType(std::string(sym->name.view()), 0, 0, 0); // Export TypeAlias as type
        }
    };

    for (const Symbol* sym : exports) {
        visit(visit, sym);
    }
}

uint32_t MetadataBuilder::addNamespace(const std::string& name, uint32_t parentNamespaceID) {
    uint32_t id = static_cast<uint32_t>(namespaces.size());
    InternalNamespace in;
    in.name = name;
    in.entry.parentNamespaceID = parentNamespaceID;
    namespaces.push_back(in);
    stringTable.addString(name);
    return id;
}

uint32_t MetadataBuilder::addType(const std::string& name, uint32_t namespaceID, uint64_t size, uint64_t alignment) {
    uint32_t id = static_cast<uint32_t>(types.size());
    InternalType in;
    in.name = name;
    in.entry.namespaceID = namespaceID;
    in.entry.size = size;
    in.entry.alignment = alignment;
    in.entry.visibility = static_cast<uint8_t>(Visibility::Public);
    in.entry.moduleID = 0;
    types.push_back(in);
    stringTable.addString(name);
    return id;
}

uint32_t MetadataBuilder::addTrait(const std::string& name, uint32_t namespaceID) {
    uint32_t id = static_cast<uint32_t>(traits.size());
    InternalTrait in;
    in.name = name;
    in.entry.namespaceID = namespaceID;
    in.entry.visibility = static_cast<uint8_t>(Visibility::Public);
    in.entry.moduleID = 0;
    traits.push_back(in);
    stringTable.addString(name);
    return id;
}

uint32_t MetadataBuilder::addFunction(const std::string& name, uint32_t namespaceID, uint32_t signatureTypeID) {
    uint32_t id = static_cast<uint32_t>(functions.size());
    InternalFunction in;
    in.name = name;
    in.entry.namespaceID = namespaceID;
    in.entry.signatureTypeID = signatureTypeID;
    in.entry.isVariadic = 0;
    in.entry.paramCount = 0;
    in.entry.flags = 0;
    in.entry.visibility = static_cast<uint8_t>(Visibility::Public);
    in.entry.moduleID = 0;
    functions.push_back(in);
    stringTable.addString(name);
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
    writer.writeU32(static_cast<uint32_t>(namespaces.size()));
    for (const auto& in : namespaces) {
        NamespaceEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }

    writer.writeU32(static_cast<uint32_t>(types.size()));
    for (const auto& in : types) {
        TypeEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }

    writer.writeU32(static_cast<uint32_t>(traits.size()));
    for (const auto& in : traits) {
        TraitEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }

    writer.writeU32(static_cast<uint32_t>(functions.size()));
    for (const auto& in : functions) {
        FunctionEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }
}

void MetadataBuilder::serializeImpls(BinaryWriter& writer) const {
    writer.writeU32(static_cast<uint32_t>(impls.size()));
    for (const auto& entry : impls) writer.writeStruct(entry);
}

} // namespace mlib
} // namespace fl
