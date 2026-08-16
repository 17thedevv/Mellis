#include "mellis/MLib/StringTableBuilder.h"
#include <cstring>
#include <stdexcept>

namespace fl {
namespace mlib {

StringTableBuilder::StringTableBuilder() {
    // Add empty string explicitly, which will end up at offset 0 after finalize()
    pendingStrings.insert("");
}

void StringTableBuilder::addString(const std::string& str) {
    if (finalized) {
        throw std::runtime_error("Cannot add string after StringTableBuilder is finalized.");
    }
    pendingStrings.insert(str);
}

void StringTableBuilder::finalize() {
    if (finalized) return;
    finalized = true;

    // Buffer should start with null terminator for empty string
    buffer.push_back(0);
    stringMap[""] = 0;

    for (const auto& str : pendingStrings) {
        if (str.empty()) continue;

        uint32_t offset = static_cast<uint32_t>(buffer.size());
        stringMap[str] = offset;

        buffer.insert(buffer.end(), str.begin(), str.end());
        buffer.push_back(0); // Null terminator
    }
}

uint32_t StringTableBuilder::getStringOffset(const std::string& str) const {
    if (!finalized) {
        throw std::runtime_error("Cannot get string offset before StringTableBuilder is finalized.");
    }
    auto it = stringMap.find(str);
    if (it != stringMap.end()) {
        return it->second;
    }
    return 0; // Return 0 for missing string (empty string)
}

void StringTableBuilder::serialize(BinaryWriter& writer) const {
    if (!finalized) {
        throw std::runtime_error("Cannot serialize StringTableBuilder before finalize().");
    }
    writer.writeBytes(buffer.data(), buffer.size());
}

size_t StringTableBuilder::getSize() const {
    return buffer.size();
}

} // namespace mlib
} // namespace fl
