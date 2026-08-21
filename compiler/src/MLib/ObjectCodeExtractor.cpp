#include "mellis/MLib/ObjectCodeExtractor.h"
#include <stdexcept>

namespace fl {
namespace mlib {

ObjectCodeExtractor::ObjectCodeExtractor(const uint8_t* data, size_t size)
    : sectionData(data), sectionSize(size) {}

void ObjectCodeExtractor::parseIndexTable() {
    BinaryReader reader(sectionData, sectionSize);

    // Read version
    uint32_t version = reader.readU32();
    if (version != 1) {
        throw std::runtime_error("Unsupported Object Code Section version: "
                                 + std::to_string(version));
    }

    uint32_t count = reader.readU32();
    orderedIDs.reserve(count);

    for (uint32_t i = 0; i < count; ++i) {
        ObjectFunctionIndex idx;
        reader.readStruct(idx);

        IndexEntry entry;
        entry.offset = idx.offset;
        entry.size   = idx.size;
        entry.objectHash = idx.objectHash;

        orderedIDs.push_back(idx.functionID);
        indexTable[idx.functionID] = entry;
    }
}

std::optional<std::vector<uint8_t>> ObjectCodeExtractor::extractFunction(uint32_t functionID) {
    auto it = indexTable.find(functionID);
    if (it == indexTable.end()) return std::nullopt;

    const IndexEntry& entry = it->second;

    if (entry.offset + entry.size > sectionSize) {
        throw std::out_of_range("ObjectCodeExtractor: blob out of section bounds");
    }

    const uint8_t* blobStart = sectionData + entry.offset;
    return std::vector<uint8_t>(blobStart, blobStart + entry.size);
}

std::vector<uint8_t> ObjectCodeExtractor::extractAll() {
    std::vector<uint8_t> result;

    for (uint32_t id : orderedIDs) {
        auto bytes = extractFunction(id);
        if (bytes) {
            result.insert(result.end(), bytes->begin(), bytes->end());
        }
    }

    return result;
}

std::optional<uint64_t> ObjectCodeExtractor::getHash(uint32_t functionID) const {
    auto it = indexTable.find(functionID);
    if (it == indexTable.end()) return std::nullopt;
    return it->second.objectHash;
}

} // namespace mlib
} // namespace fl
