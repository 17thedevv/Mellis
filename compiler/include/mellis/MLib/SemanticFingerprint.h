// =============================================================================
// mellis/MLib/SemanticFingerprint.h
//
// Computes a deterministic semantic fingerprint for the exported API
// of a module. Used for skipping recompilation of dependent modules
// when the public API has not changed.
// =============================================================================

#pragma once

#include "mellis/MiddleEnd/SemanticSnapshot.h"
#include <string>

namespace fl {
namespace mlib {

class SemanticFingerprint {
public:
    /// Computes a canonical hash of the module's public/exported API.
    /// Only symbols with Visibility::Public are considered.
    /// Returns a 64-bit FNV-1a hash formatted as a 16-character hex string.
    static std::string compute(const SemanticSnapshot& snapshot);

    /// Computes a structural hash for a single symbol using a deep semantic traversal
    /// without relying on debug representations like Type::toString().
    static uint64_t computeCanonicalHash(const Symbol& sym, const SemanticSnapshot& snapshot);
};

} // namespace mlib
} // namespace fl
