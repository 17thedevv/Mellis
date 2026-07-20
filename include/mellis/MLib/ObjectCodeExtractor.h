#ifndef MELLIS_MLIB_OBJECTCODEEXTRACTOR_H
#define MELLIS_MLIB_OBJECTCODEEXTRACTOR_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryReader.h"
#include <vector>
#include <unordered_map>
#include <cstdint>
#include <optional>

namespace fl {
namespace mlib {

class ObjectCodeExtractor {
public:
    // Initialize over an existing Object Code Section memory buffer.
    ObjectCodeExtractor(const uint8_t* data, size_t size);

    // Parse the index table. Must be called before any extract*.
    void parseIndexTable();

    // Lazy-load a single function's object bytes by FunctionID.
    // Returns empty optional if FunctionID not found.
    std::optional<std::vector<uint8_t>> extractFunction(uint32_t functionID);

    // Extract all object blobs concatenated in index order.
    // Ready to pass to an in-memory LLD linking call.
    std::vector<uint8_t> extractAll();

    // Get the stored hash for a FunctionID (for incremental linking checks).
    std::optional<uint64_t> getHash(uint32_t functionID) const;

    size_t getFunctionCount() const { return indexTable.size(); }

private:
    const uint8_t* sectionData;
    size_t sectionSize;

    struct IndexEntry {
        uint64_t offset;
        uint32_t size;
        uint64_t objectHash;
    };

    // Preserve insertion order for extractAll()
    std::vector<uint32_t> orderedIDs;
    std::unordered_map<uint32_t, IndexEntry> indexTable;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_OBJECTCODEEXTRACTOR_H
