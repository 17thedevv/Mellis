#ifndef MELLIS_MLIB_GENERICSERIALIZER_H
#define MELLIS_MLIB_GENERICSERIALIZER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/IR/MVIR.h"
#include <vector>

namespace fl {
namespace mlib {

class GenericSerializer {
public:
    GenericSerializer();

    // Serializes a single MVIR Function into a Generic Package format
    // Returns the byte vector of the serialized package.
    // - genericParamTypeIDs: type parameter IDs (T, U, ...)
    // - lifetimeParamIDs:   lifetime parameter IDs ('a, 'b, ...)
    // - constraintTraitIDs: trait constraint IDs
    std::vector<uint8_t> serializePackage(
        uint32_t functionID,
        const std::vector<uint32_t>& genericParamTypeIDs,
        const std::vector<uint32_t>& lifetimeParamIDs,
        const std::vector<uint32_t>& constraintTraitIDs,
        uint32_t returnTypeID,
        const mvir::Function& function
    );

private:
    void serializeOperand(BinaryWriter& writer, const mvir::Operand& op);
    void serializeInstruction(BinaryWriter& writer, const mvir::Instruction& inst);
    void serializeTerminator(BinaryWriter& writer, const mvir::Terminator& term);
    
    // Hash calculations
    uint64_t calculateMVIRHash(const mvir::Function& function);
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_GENERICSERIALIZER_H
