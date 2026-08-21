#include "mellis/Core/WellKnownTraits.h"

namespace fl {

WellKnownTrait WellKnownTraitRegistry::identifyTrait(const std::string& mangledName, const std::string& traitName) {
    if (traitName == "Copy") {
        return WellKnownTrait::Copy;
    }
    if (traitName == "Clone") {
        return WellKnownTrait::Clone;
    }
    if (traitName == "Drop") {
        return WellKnownTrait::Drop;
    }
    if (traitName == "Add") {
        return WellKnownTrait::Add;
    }
    if (traitName == "Sub") {
        return WellKnownTrait::Sub;
    }
    if (traitName == "Mul") {
        return WellKnownTrait::Mul;
    }
    if (traitName == "Div") {
        return WellKnownTrait::Div;
    }
    if (traitName == "Rem") {
        return WellKnownTrait::Rem;
    }
    if (traitName == "Eq") {
        return WellKnownTrait::Eq;
    }
    if (traitName == "Ord") {
        return WellKnownTrait::Ord;
    }
    if (traitName == "Index") {
        return WellKnownTrait::Index;
    }
    if (traitName == "IndexMut") {
        return WellKnownTrait::IndexMut;
    }
    
    return WellKnownTrait::Unknown;
}

} // namespace fl
