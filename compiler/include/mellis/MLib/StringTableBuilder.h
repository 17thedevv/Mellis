#ifndef MELLIS_MLIB_STRINGTABLEBUILDER_H
#define MELLIS_MLIB_STRINGTABLEBUILDER_H

#include "mellis/MLib/BinaryWriter.h"
#include <string>
#include <map>
#include <vector>
#include <set>
#include <cstdint>

namespace fl {
namespace mlib {

class StringTableBuilder {
public:
    StringTableBuilder();

    // Adds a string to the pending set. Does not return an ID/offset yet.
    void addString(const std::string& str);

    // Sorts all pending strings alphabetically, assigns them offsets, and builds the buffer.
    void finalize();

    // Retrieves the byte offset (StringID) for a string. Must be called after finalize().
    uint32_t getStringOffset(const std::string& str) const;

    // Serializes the entire string table into the provided writer
    void serialize(BinaryWriter& writer) const;

    // Get the current size of the string table block
    size_t getSize() const;

private:
    std::set<std::string> pendingStrings;
    std::map<std::string, uint32_t> stringMap;
    std::vector<uint8_t> buffer;
    bool finalized = false;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_STRINGTABLEBUILDER_H
