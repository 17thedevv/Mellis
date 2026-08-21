#ifndef MELLIS_MLIB_GENERICDESERIALIZER_H
#define MELLIS_MLIB_GENERICDESERIALIZER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryReader.h"
#include "mellis/IR/MVIR.h"
#include <vector>
#include <memory>
#include <optional>

namespace fl {
namespace mlib {

class GenericDeserializer {
public:
    explicit GenericDeserializer(const uint8_t* data, size_t size);

    // Parses the Index Table and caches offsets
    void parseIndexTable();

    // Lazy loads a specific Generic Function Package
    std::unique_ptr<mvir::Function> loadFunction(uint32_t functionID);

    // Loads function AND returns the generic param metadata (for round-trip verification)
    // Returns nullptr if not found. Fills outTypeParamIDs and outLifetimeParamIDs.
    std::unique_ptr<mvir::Function> loadFunctionWithParams(
        uint32_t functionID,
        std::vector<uint32_t>& outTypeParamIDs,
        std::vector<uint32_t>& outLifetimeParamIDs,
        std::vector<uint32_t>& outConstraintIDs
    );

private:
    mvir::Operand deserializeOperand(BinaryReader& reader);
    std::unique_ptr<mvir::Instruction> deserializeInstruction(BinaryReader& reader);
    std::unique_ptr<mvir::Terminator> deserializeTerminator(BinaryReader& reader);

    const uint8_t* sectionData;
    size_t sectionSize;
    
    struct IndexEntry {
        uint64_t offset;
        uint32_t compressedSize;
        uint32_t uncompressedSize;
    };
    
    // functionID -> IndexEntry
    std::unordered_map<uint32_t, IndexEntry> indexTable;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_GENERICDESERIALIZER_H
