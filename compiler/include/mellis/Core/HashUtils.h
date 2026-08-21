// =============================================================================
// mellis/Core/HashUtils.h
//
// Lightweight, zero-dependency hashing utilities for the Incremental Build system.
//
// Design:
//   - FNV-1a 64-bit: ~10 ns/KB, zero allocation, constexpr-friendly.
//   - No crypto guarantees needed — we only need good collision resistance
//     for file content fingerprinting.
//   - combineHashes() implements the "Avalanche Effect": changing any single
//     dependency's hash cascades through to the parent's combined hash.
//
// FNV-1a algorithm:
//   hash = FNV_OFFSET_BASIS
//   for each byte b in data:
//       hash = hash XOR b
//       hash = hash * FNV_PRIME
//
// Reference: http://www.isthe.com/chongo/tech/comp/fnv/
// =============================================================================

#pragma once
#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

namespace fl {

// FNV-1a 64-bit constants
constexpr uint64_t FNV1A_OFFSET_BASIS = 14695981039346656037ULL;
constexpr uint64_t FNV1A_PRIME        = 1099511628211ULL;

// -----------------------------------------------------------------------------
// fnv1a64 — hash a contiguous block of bytes
// O(n), no heap allocation, safe for any string_view including binary data.
// -----------------------------------------------------------------------------
inline uint64_t fnv1a64(std::string_view data) noexcept {
    uint64_t hash = FNV1A_OFFSET_BASIS;
    for (unsigned char c : data) {
        hash ^= static_cast<uint64_t>(c);
        hash *= FNV1A_PRIME;
    }
    return hash;
}

// -----------------------------------------------------------------------------
// combineHashes — mix a dependency hash into a running accumulator.
//
// Uses the "xor-shift-multiply" pattern to ensure:
//   - No dependency hash is identity-neutral (XOR with 0 is a no-op → bad).
//   - Order of combination matters (not commutative → different dep order =
//     different combined hash, which is correct since build order is fixed).
//
// This is equivalent to what Bazel and Cargo do for their "fingerprint chaining".
// -----------------------------------------------------------------------------
inline uint64_t combineHashes(uint64_t base, uint64_t dep) noexcept {
    // Knuth multiplicative hash + XOR — provides good avalanche effect
    dep ^= dep >> 33;
    dep *= 0xff51afd7ed558ccdULL;
    dep ^= dep >> 33;
    dep *= 0xc4ceb9fe1a85ec53ULL;
    dep ^= dep >> 33;
    return base ^ dep;
}

// -----------------------------------------------------------------------------
// calculateModuleHash — the "Avalanche" hash for a single module.
//
// The final hash encodes:
//   1. The content of the module's own source file.
//   2. The combined hash of all its direct dependency modules
//      (which themselves encode their dependencies, recursively).
//
// Property: If ANY transitive dependency changes, this hash changes.
// This is the "Snowball Effect" (Hiệu ứng tuyết lở) that enables safe
// incremental compilation.
//
// @param sourceCode  Raw content of the .ms source file.
// @param depHashes   The `currentHash` of each dependency module,
//                    ordered by their appearance in the topological sort.
// @return            A uint64_t fingerprint of this module + its full dep chain.
// -----------------------------------------------------------------------------
inline uint64_t calculateModuleHash(std::string_view sourceCode,
                                     const std::vector<uint64_t>& depHashes) noexcept {
    uint64_t hash = fnv1a64(sourceCode);
    for (uint64_t dep : depHashes) {
        hash = combineHashes(hash, dep);
    }
    return hash;
}

} // namespace fl
