#include "mellis/MLib/ObjectCodeBuilder.h"

namespace fl {
namespace mlib {

ObjectCodeBuilder::ObjectCodeBuilder() {}

// FNV-1a 64-bit hash — fast, deterministic, zero-dependency
// Production upgrade path: drop-in replace with XXHash64
uint64_t ObjectCodeBuilder::computeHash(const uint8_t* data, size_t size) {
    uint64_t hash = 14695981039346656037ULL; // FNV offset basis
    for (size_t i = 0; i < size; ++i) {
        hash ^= static_cast<uint64_t>(data[i]);
        hash *= 1099511628211ULL; // FNV prime
    }
    return hash;
}

void ObjectCodeBuilder::addFunction(uint32_t functionID, const uint8_t* objData, size_t size) {
    Entry entry;
    entry.functionID = functionID;
    entry.objectHash = computeHash(objData, size);
    entry.blob.assign(objData, objData + size);
    entries.push_back(std::move(entry));
}

void ObjectCodeBuilder::serialize(BinaryWriter& writer) const {
    // Section Version
    writer.writeU32(1);

    // Function Count
    writer.writeU32(static_cast<uint32_t>(entries.size()));

    // ── Build the Index Table ─────────────────────────────────────────────────
    // We need to know each blob's offset. The data region starts immediately
    // after the index table. Calculate data-start offset first.
    //
    // Layout (from section start):
    //   Version(4) + Count(4) + [N * sizeof(ObjectFunctionIndex)]
    //   = 8 + N * 24 bytes
    //
    const size_t indexTableBytes = entries.size() * sizeof(ObjectFunctionIndex);
    const uint64_t dataStartOffset = 8 + indexTableBytes;

    uint64_t currentOffset = dataStartOffset;

    // Write Index Table
    for (const auto& e : entries) {
        ObjectFunctionIndex idx;
        idx.functionID = e.functionID;
        idx.size = static_cast<uint32_t>(e.blob.size());
        idx.offset = currentOffset;
        idx.objectHash = e.objectHash;
        writer.writeStruct(idx);
        currentOffset += e.blob.size();
    }

    // ── Write Blobs ───────────────────────────────────────────────────────────
    for (const auto& e : entries) {
        writer.writeBytes(e.blob.data(), e.blob.size());
    }
}

} // namespace mlib
} // namespace fl
