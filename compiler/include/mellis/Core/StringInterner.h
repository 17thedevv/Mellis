// =============================================================================
// mellis/Core/StringInterner.h
//
// String interning infrastructure — MVP STUB.
//
// What string interning does:
//   Given the same string "player" three times in source code,
//   the interner returns the SAME InternID (e.g., 42) for all three.
//   This means:
//     - Zero duplicate heap allocations for repeated identifiers.
//     - Identifier equality becomes O(1): `id_a == id_b` vs string compare.
//     - The hash of an identifier is computed ONCE at intern time.
//
// Architecture contract:
//   Identifier (see Identifier.h) wraps the name as std::string today.
//   When this interner is fully implemented:
//     1. Identifier stores InternID internally (not std::string).
//     2. Identifier constructor calls interner.intern(str).
//     3. Identifier::operator== becomes `id_ == other.id_` (integer compare).
//     4. No call-sites change — the Identifier public API is stable.
//
// Upgrade path (when needed):
//   Implement intern() using an unordered_map<string, InternID> + vector<string>.
//   Pass StringInterner& into Identifier constructor (or make it a global).
//
// Not needed for MVP because:
//   - Single-file compiler → identifiers are small in number.
//   - Correctness is not affected.
//   - The architecture already reserves the right slot for this.
// =============================================================================

#pragma once
#include "mellis/Core/Types.h"
#include <string_view>

namespace fl {

/// Opaque intern ID — wraps a uint32_t index into the intern pool.
/// Intentionally distinct from SymbolID to avoid mix-ups.
using InternID = uint32_t;
constexpr InternID kInvalidInternID = std::numeric_limits<uint32_t>::max();

/// String interner — canonical registry mapping strings to InternIDs.
///
/// MVP: class body intentionally left empty.
/// Future: implement intern() / lookup() below.
class StringInterner {
public:
    // ── Future API ───────────────────────────────────────────────────────────
    // InternID intern(std::string_view str);
    // std::string_view lookup(InternID id) const;
    // bool contains(std::string_view str) const;
};

} // namespace fl
