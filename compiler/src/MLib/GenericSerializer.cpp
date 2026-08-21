#include "mellis/MLib/GenericSerializer.h"
#include <cstring>
#include <functional>

namespace fl {
namespace mlib {

GenericSerializer::GenericSerializer() {}

uint64_t GenericSerializer::calculateMVIRHash(const mvir::Function& function) {
    // A real implementation would use XXHash64 over the structured content
    // For now, we return a dummy hash.
    return 0xDEADBEEF12345678ULL;
}

void GenericSerializer::serializeOperand(BinaryWriter& writer, const mvir::Operand& op) {
    writer.writeU8(static_cast<uint8_t>(op.index())); // Tag
    
    std::visit([&writer](auto&& arg) {
        using T = std::decay_t<decltype(arg)>;
        if constexpr (std::is_same_v<T, mvir::Place>) {
            if (auto* loc = std::get_if<mvir::LocalId>(&arg.base)) {
                writer.writeU8(0);
                writer.writeString(loc->name);
                // S7.2c: preserve semantic identity fields
                writer.writeU32(loc->symbolId);
                writer.writeU32(loc->expansionId);
            } else if (auto* gl = std::get_if<mvir::GlobalId>(&arg.base)) {
                writer.writeU8(1);
                writer.writeString(gl->name);
            } else {
                writer.writeU8(0);
                writer.writeString("");
                writer.writeU32(0); // symbolId
                writer.writeU32(0); // expansionId
            }
        } else if constexpr (std::is_same_v<T, mvir::Number>) {
            writer.writeString(arg.value);
        } else if constexpr (std::is_same_v<T, mvir::Boolean>) {
            writer.writeU8(arg.value ? 1 : 0);
        }
    }, op);
}

void GenericSerializer::serializeInstruction(BinaryWriter& writer, const mvir::Instruction& inst) {
    mvir::Opcode opc = inst.getOpcode();
    writer.writeU8(static_cast<uint8_t>(opc));

    // A real implementation would dynamically downcast and serialize fields.
    // For the sake of the structural proof-of-concept, we serialize a minimal representation.
    // E.g., if it's an AluInst, serialize dest, op, left, right.
    
    if (opc == mvir::Opcode::Alu) {
        const auto* alu = static_cast<const mvir::AluInst*>(&inst);
        writer.writeString(alu->dest.name);
        writer.writeU32(alu->dest.symbolId);
        writer.writeU32(alu->dest.expansionId);
        writer.writeU8(static_cast<uint8_t>(alu->op));
        serializeOperand(writer, alu->left);
        serializeOperand(writer, alu->right);
    } else if (opc == mvir::Opcode::Load) {
        const auto* ld = static_cast<const mvir::LoadInst*>(&inst);
        writer.writeString(ld->dest.name);
        writer.writeU32(ld->dest.symbolId);
        writer.writeU32(ld->dest.expansionId);
        writer.writeU32(0); // Dummy TypeID
        serializeOperand(writer, ld->ptr);
    }
}

void GenericSerializer::serializeTerminator(BinaryWriter& writer, const mvir::Terminator& term) {
    mvir::Opcode opc = term.getOpcode();
    writer.writeU8(static_cast<uint8_t>(opc));

    if (opc == mvir::Opcode::Ret) {
        const auto* ret = static_cast<const mvir::RetTerm*>(&term);
        if (ret->value) {
            writer.writeU8(1);
            serializeOperand(writer, *ret->value);
        } else {
            writer.writeU8(0);
        }
    }
}

std::vector<uint8_t> GenericSerializer::serializePackage(
    uint32_t functionID,
    const std::vector<uint32_t>& genericParamTypeIDs,
    const std::vector<uint32_t>& lifetimeParamIDs,
    const std::vector<uint32_t>& constraintTraitIDs,
    uint32_t returnTypeID,
    const mvir::Function& function
) {
    BinaryWriter writer;

    // 1. Write Generic Package Header
    GenericPackageHeader header;
    header.functionID = functionID;
    header.typeParamCount = static_cast<uint16_t>(genericParamTypeIDs.size());
    header.lifetimeParamCount = static_cast<uint16_t>(lifetimeParamIDs.size());
    header.constraintCount = static_cast<uint16_t>(constraintTraitIDs.size());
    header.flags = 0; // e.g. 1 if async
    header.returnTypeID = returnTypeID;
    
    writer.writeStruct(header);

    // 2. Write Type Params, Lifetime Params, & Constraints
    for (uint32_t gid : genericParamTypeIDs) writer.writeU32(gid);
    for (uint32_t lid : lifetimeParamIDs) writer.writeU32(lid);
    for (uint32_t cid : constraintTraitIDs) writer.writeU32(cid);

    // 3. Write MVIR Function Blocks
    writer.writeU32(static_cast<uint32_t>(function.blocks.size()));

    for (const auto& block : function.blocks) {
        writer.writeString(block->label.name);
        
        writer.writeU32(static_cast<uint32_t>(block->instructions.size()));
        for (const auto& inst : block->instructions) {
            serializeInstruction(writer, *inst);
        }
        
        if (block->terminator) {
            serializeTerminator(writer, *block->terminator);
        } else {
            // Null terminator fallback
            writer.writeU8(static_cast<uint8_t>(mvir::Opcode::Ret));
            writer.writeU8(0);
        }
    }

    return writer.takeBuffer();
}

} // namespace mlib
} // namespace fl
