#include "mellis/MLib/DebugInfoBuilder.h"

namespace fl {
namespace mlib {

// ──────────────────────────────────────────────────────────────────────────────
// DebugInfoBuilder
// ──────────────────────────────────────────────────────────────────────────────

void DebugInfoBuilder::addLineEntry(uint32_t functionID, uint32_t instructionIndex,
                                    uint32_t fileStringID, uint32_t line, uint32_t column) {
    DebugLineEntry e;
    e.functionID       = functionID;
    e.instructionIndex = instructionIndex;
    e.fileStringID     = fileStringID;
    e.line             = line;
    e.column           = column;
    entries.push_back(e);
}

void DebugInfoBuilder::serialize(BinaryWriter& writer) const {
    writer.writeU32(1); // Version
    writer.writeU32(static_cast<uint32_t>(entries.size()));
    for (const auto& e : entries) {
        writer.writeStruct(e);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DebugInfoReader
// ──────────────────────────────────────────────────────────────────────────────

DebugInfoReader::DebugInfoReader(const uint8_t* data, size_t size)
    : sectionData(data), sectionSize(size) {}

void DebugInfoReader::parse() {
    BinaryReader reader(sectionData, sectionSize);
    uint32_t version = reader.readU32();
    if (version != 1) {
        throw std::runtime_error("Unsupported Debug Section version");
    }
    uint32_t count = reader.readU32();
    entries.resize(count);
    for (uint32_t i = 0; i < count; ++i) {
        reader.readStruct(entries[i]);
    }
}

std::optional<DebugInfoReader::SourceLoc>
DebugInfoReader::lookup(uint32_t functionID, uint32_t instructionIndex) const {
    for (const auto& e : entries) {
        if (e.functionID == functionID && e.instructionIndex == instructionIndex) {
            return SourceLoc{e.fileStringID, e.line, e.column};
        }
    }
    return std::nullopt;
}

} // namespace mlib
} // namespace fl
