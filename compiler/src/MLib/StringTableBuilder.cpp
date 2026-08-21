#include "mellis/MLib/StringTableBuilder.h"
#include <cstring>
#include <stdexcept>

namespace fl {
namespace mlib {

StringTableBuilder::StringTableBuilder() {
    buffer.push_back(0);
    stringMap[""] = 0;
}

uint32_t StringTableBuilder::addString(const std::string& str) {
    if (finalized) {
        throw std::runtime_error("Cannot add string after StringTableBuilder is finalized.");
    }
    if (str.empty()) return 0;
    
    auto it = stringMap.find(str);
    if (it != stringMap.end()) {
        return it->second;
    }
    
    uint32_t offset = static_cast<uint32_t>(buffer.size());
    stringMap[str] = offset;
    buffer.insert(buffer.end(), str.begin(), str.end());
    buffer.push_back(0); // Null terminator
    return offset;
}

void StringTableBuilder::finalize() {
    finalized = true;
}

uint32_t StringTableBuilder::getStringOffset(const std::string& str) const {
    auto it = stringMap.find(str);
    if (it != stringMap.end()) {
        return it->second;
    }
    return 0; 
}

void StringTableBuilder::serialize(BinaryWriter& writer) const {
    writer.writeBytes(buffer.data(), buffer.size());
}

size_t StringTableBuilder::getSize() const {
    return buffer.size();
}

} // namespace mlib
} // namespace fl
