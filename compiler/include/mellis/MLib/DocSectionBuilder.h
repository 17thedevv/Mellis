#ifndef MELLIS_MLIB_DOCSECTIONBUILDER_H
#define MELLIS_MLIB_DOCSECTIONBUILDER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/BinaryReader.h"
#include "mellis/MLib/StringTableBuilder.h"
#include <vector>
#include <unordered_map>
#include <string>
#include <optional>
#include <cstdint>

namespace fl {
namespace mlib {

// DocSectionBuilder: stores doc-comment text keyed by SymbolID.
// Section layout: [Version(u32)] [Count(u32)] [DocEntry * Count]
// StringIDs refer to the module's shared String Table.
class DocSectionBuilder {
public:
    explicit DocSectionBuilder(StringTableBuilder& stringTable);

    // Associate a doc comment string with a symbolID (FunctionID or TypeID).
    void addDoc(uint32_t symbolID, const std::string& docComment);

    void serialize(BinaryWriter& writer) const;
    size_t getEntryCount() const { return entries.size(); }

private:
    StringTableBuilder& stringTable;
    struct InternalDocEntry {
        uint32_t symbolID;
        std::string docComment;
    };
    std::vector<InternalDocEntry> entries;
};

// DocSectionReader: parses a Doc Section and resolves StringIDs
// against a raw string table buffer for zero-copy hover text.
class DocSectionReader {
public:
    // stringTableData: pointer to the raw StringTable section bytes
    DocSectionReader(const uint8_t* data, size_t size,
                     const uint8_t* stringTableData, size_t stringTableSize);

    void parse();

    // Returns the doc comment string for a symbolID, or nullopt if absent.
    std::optional<std::string> getDoc(uint32_t symbolID) const;
    size_t getEntryCount() const { return docMap.size(); }

private:
    const uint8_t* sectionData;
    size_t sectionSize;
    const uint8_t* stringTableData;
    size_t stringTableSize;

    // symbolID -> doc comment text (resolved at parse time)
    std::unordered_map<uint32_t, std::string> docMap;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_DOCSECTIONBUILDER_H
