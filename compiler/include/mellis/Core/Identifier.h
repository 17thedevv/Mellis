// =============================================================================
// mellis/Core/Identifier.h
//
// The Identifier type: a named entity in source code.
//
// Design rationale:
//   The fundamental problem with storing raw std::string everywhere:
//     - "player" declared 1000 times → 1000 heap allocations
//     - Equality checks → O(n) string comparisons
//     - Hash-map lookups → O(n) hashing of full string each time
//
//   Production compilers solve this with string interning:
//     rustc   → Symbol (wraps u32 InternID)
//     Clang   → IdentifierInfo* (pointer into a hash table)
//     Swift   → Identifier (wraps StringRef into an interned pool)
//
//   Our Identifier type wraps std::string for the MVP. The PUBLIC API
//   is intentionally identical to what it will be after full interning:
//     - Identifier(string_view) — construct from source text
//     - view()                  — read-only access
//     - operator==, hash()      — for use in maps and sets
//
//   When StringInterner is fully implemented (see StringInterner.h):
//     1. Add InternID as the internal storage field.
//     2. Change the constructor to call interner.intern(str).
//     3. Change operator== to `this->id_ == other.id_` — O(1).
//     4. No changes to any call-site.
//
// MVP: string-backed, correct, slightly slower than interned.
// =============================================================================

#pragma once
#include <string>
#include <string_view>
#include <functional>   // std::hash

namespace fl {

class Identifier {
public:
    // ── Construction ─────────────────────────────────────────────────────────

    Identifier() = default;

    /// Primary constructor: accepts source text (string_view = zero-copy from AST).
    explicit Identifier(std::string_view sv) : name_(sv) {}

    /// Convenience: construct from std::string (takes ownership).
    explicit Identifier(std::string str) : name_(std::move(str)) {}

    // ── Observers ────────────────────────────────────────────────────────────

    std::string_view  view()  const { return name_; }
    const std::string& str()  const { return name_; }
    bool              empty() const { return name_.empty(); }

    // ── Comparison ───────────────────────────────────────────────────────────

    bool operator==(const Identifier& other) const {
        return name_ == other.name_;
    }
    bool operator!=(const Identifier& other) const {
        return !(*this == other);
    }
    bool operator==(std::string_view sv) const { return name_ == sv; }
    bool operator!=(std::string_view sv) const { return name_ != sv; }

private:
    // MVP: owned string.
    // Future: replace with `InternID id_` — all observers updated internally.
    std::string name_;
};

// ── Hash / Equal helpers for use in ScopeBindings ────────────────────────────
//
// is_transparent enables heterogeneous lookup:
//   map.find("player")          — no Identifier construction needed
//   map.find(string_view_var)   — same
// This is critical for hot lookup paths.

struct IdentifierHash {
    using is_transparent = void;

    size_t operator()(const Identifier& id) const noexcept {
        return std::hash<std::string_view>{}(id.view());
    }
    size_t operator()(std::string_view sv) const noexcept {
        return std::hash<std::string_view>{}(sv);
    }
    size_t operator()(const std::string& s) const noexcept {
        return std::hash<std::string_view>{}(s);
    }
};

struct IdentifierEqual {
    using is_transparent = void;

    bool operator()(const Identifier& a, const Identifier& b) const noexcept {
        return a == b;
    }
    bool operator()(const Identifier& a, std::string_view b) const noexcept {
        return a.view() == b;
    }
    bool operator()(std::string_view a, const Identifier& b) const noexcept {
        return a == b.view();
    }
};

} // namespace fl
