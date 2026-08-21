// =============================================================================
// mellis/MiddleEnd/SemanticSnapshot.h
//
// SemanticSnapshot - An immutable snapshot of the semantic state (types, etc.)
// after TypeChecker verification.
// =============================================================================
#pragma once

#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include <vector>

namespace fl {

class ProgramNode; // Forward declaration

class SemanticSnapshot {
public:
    SemanticSnapshot(std::vector<const Type*> typeTable, const TypeContext& typeCtx, const SymbolTable& symTab, const ProgramNode* program = nullptr)
        : typeTable_(std::move(typeTable)), typeCtx_(typeCtx), symTab_(symTab), program_(program) {}

    const Type* typeOf(SymbolID id) const {
        if (id < typeTable_.size()) {
            return typeTable_[id];
        }
        return nullptr;
    }

    const TypeContext& getContext() const { return typeCtx_; }
    const SymbolTable& getSymbolTable() const { return symTab_; }
    const ProgramNode* getProgram() const { return program_; }

    const std::vector<const Type*>& getTypeTable() const { return typeTable_; }

private:
    std::vector<const Type*> typeTable_;
    const TypeContext& typeCtx_;
    const SymbolTable& symTab_;
    const ProgramNode* program_;
};

} // namespace fl
