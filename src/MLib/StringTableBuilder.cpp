#include "mellis/MLib/StringTableBuilder.h"
#include <cstring>

namespace fl {
namespace mlib {

StringTableBuilder::StringTableBuilder() {
    // Add an empty string at offset 0.
    // This allows StringID = 0 to mean "empty" or "null".
    buffer.push_back(0); 
    stringMap[""] = 0;
}

uint32_t StringTableBuilder::addString(const std::string& str) {
    if (str.empty()) {
        return 0;
    }

    auto it = stringMap.find(str);
    if (it != stringMap.end()) {
        return it->second;
    }

    // Offset in the buffer where this string will start
    uint32_t offset = static_cast<uint32_t>(buffer.size());
    
    // Store in map
    stringMap[str] = offset;

    // We store strings as null-terminated in the string table to make it easy for
    // O(1) resolution via simple pointer arithmetic: `const char* str = table + offset;`
    buffer.insert(buffer.end(), str.begin(), str.end());
    buffer.push_back(0); // Null terminator

    return offset;
}

void StringTableBuilder::serialize(BinaryWriter& writer) const {
    writer.writeBytes(buffer.data(), buffer.size());
}

size_t StringTableBuilder::getSize() const {
    return buffer.size();
}

} // namespace mlib
} // namespace fl
