#ifndef MELLIS_MLIB_OBJECTCODEBUILDER_H
#define MELLIS_MLIB_OBJECTCODEBUILDER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include <vector>
#include <cstdint>

namespace fl {
namespace mlib {

class ObjectCodeBuilder {
public:
    ObjectCodeBuilder();

    // Add a compiled object blob for a specific function.
    // objData is the raw COFF/ELF bytes from the backend.
    void addFunction(uint32_t functionID, const uint8_t* objData, size_t size);

    // Serialize the complete Object Code Section to the writer.
    // Layout: [Version(u32)] [Count(u32)] [IndexTable] [Blobs...]
    void serialize(BinaryWriter& writer) const;

    size_t getFunctionCount() const { return entries.size(); }

private:
    // Simple FNV-1a hash for object bytes (production would use XXHash64)
    static uint64_t computeHash(const uint8_t* data, size_t size);

    struct Entry {
        uint32_t functionID;
        uint64_t objectHash;
        std::vector<uint8_t> blob;
    };

    std::vector<Entry> entries;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_OBJECTCODEBUILDER_H
