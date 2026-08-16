#pragma once
#include <vector>
#include <unordered_map>
#include <string>
#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/SymbolTable.h"

namespace fl {

struct VTableLayout {
    size_t size = 0;
    size_t align = 0;
    SymbolID dropPtr = kInvalidSymbolID;
    
    // Map from Method SymbolID (semantic identity) to Slot Index in the VTable
    std::unordered_map<SymbolID, size_t> methodSlots;
    
    // Ordered list of method IDs representing the VTable slots
    std::vector<SymbolID> slotOrder;
};

class TraitObjectLayoutBuilder {
public:
    TraitObjectLayoutBuilder(const SymbolTable& symTable);

    // Calculates and caches the layout for a specific Trait Object
    const VTableLayout& getOrCreateLayout(const TraitObjectType* traitObjType);

    // Gets a stable, mangled name for the global VTable array instance
    // combining the concrete type and the canonical trait object type.
    std::string getVTableMangledName(const Type* concreteType, const TraitObjectType* traitObjType) const;

    const SymbolTable& getSymbolTable() const { return symTable_; }

private:
    const SymbolTable& symTable_;
    std::unordered_map<const TraitObjectType*, VTableLayout> layouts_;
};

} // namespace fl
