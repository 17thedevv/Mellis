#ifndef MELLIS_MLIB_DEBUGINFOBUILDER_H
#define MELLIS_MLIB_DEBUGINFOBUILDER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/BinaryReader.h"
#include <vector>
#include <cstdint>
#include <optional>

namespace fl {
namespace mlib {

// DebugInfoBuilder: accumulates DebugLineEntry records and serializes them.
// Section layout: [Version(u32)] [Count(u32)] [DebugLineEntry * Count]
class DebugInfoBuilder {
public:
    void addLineEntry(uint32_t functionID, uint32_t instructionIndex,
                      uint32_t fileStringID, uint32_t line, uint32_t column);

    void serialize(BinaryWriter& writer) const;
    size_t getEntryCount() const { return entries.size(); }

private:
    std::vector<DebugLineEntry> entries;
};

// DebugInfoReader: parses Debug Section and answers line-mapping queries.
class DebugInfoReader {
public:
    DebugInfoReader(const uint8_t* data, size_t size);
    void parse();

    // Returns the source location for a given (functionID, instructionIndex).
    // Returns nullopt if not found.
    struct SourceLoc { uint32_t fileStringID, line, column; };
    std::optional<SourceLoc> lookup(uint32_t functionID, uint32_t instructionIndex) const;
    size_t getEntryCount() const { return entries.size(); }

private:
    const uint8_t* sectionData;
    size_t sectionSize;
    std::vector<DebugLineEntry> entries;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_DEBUGINFOBUILDER_H
