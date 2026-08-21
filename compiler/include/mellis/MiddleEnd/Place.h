#pragma once

#include <vector>
#include <string>
#include <variant>
#include "mellis/IR/MVIR.h"

namespace fl {

enum class ProjectionKind {
    Field,
    Index,
    Deref
};

struct Projection {
    ProjectionKind kind;
    // For Field, this could be the field index or name.
    // We'll use a generic string/index. For MVP, we can just use an int index or string.
    std::string fieldName; 
    
    bool operator==(const Projection& other) const {
        return kind == other.kind && fieldName == other.fieldName;
    }
};

/// A Place represents a memory location (e.g. `x`, `x.f`, `arr[0]`, `*ptr`)
class Place {
public:
    mvir::Operand base;
    std::vector<Projection> projections;

    Place() = default;
    Place(mvir::Operand base) : base(base) {}
    Place(mvir::Operand base, std::vector<Projection> projs) : base(base), projections(std::move(projs)) {}

    bool operator==(const Place& other) const {
        if (!(base == other.base)) return false;
        if (projections.size() != other.projections.size()) return false;
        for (size_t i = 0; i < projections.size(); ++i) {
            if (!(projections[i] == other.projections[i])) return false;
        }
        return true;
    }

    /// Returns true if `this` place is a prefix of `other`.
    /// E.g. `x` is a prefix of `x.f`. `x.f` is NOT a prefix of `x`.
    bool isPrefixOf(const Place& other) const {
        if (!(base == other.base)) return false;
        if (projections.size() > other.projections.size()) return false;
        
        for (size_t i = 0; i < projections.size(); ++i) {
            if (!(projections[i] == other.projections[i])) return false;
        }
        return true;
    }

    /// Returns true if this place and `other` overlap (one is a prefix of the other).
    /// For example, borrowing `x.f` mutably overlaps with borrowing `x`.
    bool overlapsWith(const Place& other) const {
        return isPrefixOf(other) || other.isPrefixOf(*this);
    }

    std::string toString() const {
        std::string res = mvir::toString(base);
        for (const auto& p : projections) {
            if (p.kind == ProjectionKind::Field) {
                res += "." + p.fieldName;
            } else if (p.kind == ProjectionKind::Index) {
                res += "[" + p.fieldName + "]";
            } else if (p.kind == ProjectionKind::Deref) {
                res = "*" + res;
            }
        }
        return res;
    }
};

} // namespace fl
