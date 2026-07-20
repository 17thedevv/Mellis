#ifndef MELLIS_MLIB_STRINGTABLEBUILDER_H
#define MELLIS_MLIB_STRINGTABLEBUILDER_H

#include "mellis/MLib/BinaryWriter.h"
#include <string>
#include <unordered_map>
#include <vector>
#include <cstdint>

namespace fl {
namespace mlib {

class StringTableBuilder {
public:
    StringTableBuilder();

    // Adds a string and returns its byte offset (StringID) in the table.
    // If the string already exists, returns the existing ID.
    uint32_t addString(const std::string& str);

    // Serializes the entire string table into the provided writer
    void serialize(BinaryWriter& writer) const;

    // Get the current size of the string table block
    size_t getSize() const;

private:
    std::unordered_map<std::string, uint32_t> stringMap;
    std::vector<uint8_t> buffer;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_STRINGTABLEBUILDER_H
