#include "mellis/MiddleEnd/ABI/TraitObjectLayout.h"
#include <algorithm>

namespace fl {

TraitObjectLayoutBuilder::TraitObjectLayoutBuilder(const SymbolTable& symTable)
    : symTable_(symTable) {}

const VTableLayout& TraitObjectLayoutBuilder::getOrCreateLayout(const TraitObjectType* traitObjType) {
    auto it = layouts_.find(traitObjType);
    if (it != layouts_.end()) {
        return it->second;
    }

    VTableLayout layout;
    layout.size = 0; // metadata size
    layout.align = 0; // metadata align
    layout.dropPtr = kInvalidSymbolID; // metadata drop

    // Slot 0: size
    // Slot 1: align
    // Slot 2: drop function
    // Slots 3+: Methods
    size_t slotIndex = 3; 

    // The traitObjType->traitIds is ALREADY sorted and canonicalized by TypeContext.
    for (SymbolID tId : traitObjType->traitIds) {
        // Collect methods from this specific trait via SymbolTable semantic mapping
        // We do NOT use AST TraitDeclNode.
        const std::vector<SymbolID>& methods = symTable_.getTraitMethods(tId);

        // Sorting the methods by their declaration ID (SymbolID) ensures stable,
        // AST-agnostic ordering that is canonical as long as IDs are stably assigned
        // or loaded sequentially from an .mlib.
        std::vector<SymbolID> sortedMethods = methods;
        std::sort(sortedMethods.begin(), sortedMethods.end());

        for (SymbolID mId : sortedMethods) {
            layout.methodSlots[mId] = slotIndex++;
            layout.slotOrder.push_back(mId);
        }
    }

    layouts_[traitObjType] = layout;
    return layouts_[traitObjType];
}

std::string TraitObjectLayoutBuilder::getVTableMangledName(const Type* concreteType, const TraitObjectType* traitObjType) const {
    // Generate canonical identity: __vtable_Concrete_for_Trait1_Trait2
    std::string mangled = "__vtable_";
    
    // Mangle concrete type
    if (auto* st = dynamic_cast<const StructType*>(concreteType)) {
        mangled += "struct_" + std::to_string(st->structSymbolId);
    } else if (auto* pt = dynamic_cast<const PrimitiveType*>(concreteType)) {
        mangled += "prim_" + std::to_string(static_cast<int>(pt->builtinKind));
    } else if (concreteType->getKind() == TypeKind::Unknown) {
        mangled += "unknown";
    } else {
        mangled += "type_" + std::to_string(static_cast<int>(concreteType->getKind()));
    }

    mangled += "_for_dyn";

    // Mangle trait canonical set
    for (SymbolID tId : traitObjType->traitIds) {
        mangled += "_" + std::to_string(tId);
    }
    
    return mangled;
}

} // namespace fl
