#include "mellis/MLib/DocSectionBuilder.h"
#include <stdexcept>
#include <cstring>

namespace fl {
namespace mlib {

// ──────────────────────────────────────────────────────────────────────────────
// DocSectionBuilder
// ──────────────────────────────────────────────────────────────────────────────

DocSectionBuilder::DocSectionBuilder(StringTableBuilder& stringTable)
    : stringTable(stringTable) {}

void DocSectionBuilder::addDoc(uint32_t symbolID, const std::string& docComment) {
    InternalDocEntry e;
    e.symbolID = symbolID;
    e.docComment = docComment;
    entries.push_back(e);
    stringTable.addString(docComment);
}

void DocSectionBuilder::serialize(BinaryWriter& writer) const {
    writer.writeU32(1); // Version
    writer.writeU32(static_cast<uint32_t>(entries.size()));
    for (const auto& e : entries) {
        DocEntry out;
        out.symbolID = e.symbolID;
        out.docStringID = stringTable.getStringOffset(e.docComment);
        writer.writeStruct(out);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DocSectionReader
// ──────────────────────────────────────────────────────────────────────────────

DocSectionReader::DocSectionReader(const uint8_t* data, size_t size,
                                   const uint8_t* stringTableData, size_t stringTableSize)
    : sectionData(data), sectionSize(size),
      stringTableData(stringTableData), stringTableSize(stringTableSize) {}

void DocSectionReader::parse() {
    BinaryReader reader(sectionData, sectionSize);

    uint32_t version = reader.readU32();
    if (version != 1) {
        throw std::runtime_error("Unsupported Doc Section version");
    }

    uint32_t count = reader.readU32();
    for (uint32_t i = 0; i < count; ++i) {
        DocEntry e;
        reader.readStruct(e);

        // Resolve the StringID to a real C string via the string table
        uint32_t strOffset = e.docStringID;
        if (strOffset >= stringTableSize) {
            throw std::out_of_range("DocSectionReader: StringID out of bounds");
        }

        // String table is null-terminated strings stored contiguously.
        // O(1) pointer-math lookup — no heap allocation.
        const char* rawStr = reinterpret_cast<const char*>(stringTableData + strOffset);
        docMap[e.symbolID] = std::string(rawStr);
    }
}

std::optional<std::string> DocSectionReader::getDoc(uint32_t symbolID) const {
    auto it = docMap.find(symbolID);
    if (it == docMap.end()) return std::nullopt;
    return it->second;
}

} // namespace mlib
} // namespace fl
