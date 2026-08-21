#pragma once

#include <string>
#include <optional>
#include "mellis/MiddleEnd/Symbol.h"

namespace fl {

// Represents stable identities for core traits
enum class WellKnownTrait {
    Copy,
    Clone,
    Drop,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ord,
    Index,
    IndexMut,
    Unknown
};

class WellKnownTraitRegistry {
public:
    // Try to map a fully qualified symbol name (e.g. "core::Copy" or "std::clone::Clone")
    // to a WellKnownTrait enum. In a real system, this might use attributes like #[lang="copy"],
    // but for now we map known paths to the enum.
    static WellKnownTrait identifyTrait(const std::string& mangledName, const std::string& traitName);
    
    // Check if a trait is the Copy trait (the most common check)
    static bool isCopy(WellKnownTrait wkt) { return wkt == WellKnownTrait::Copy; }
    
    // Check if a trait is the Drop trait
    static bool isDrop(WellKnownTrait wkt) { return wkt == WellKnownTrait::Drop; }
};

} // namespace fl
