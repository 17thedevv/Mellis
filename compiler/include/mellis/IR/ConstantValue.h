#pragma once

#include <string>
#include <vector>
#include <unordered_map>
#include <cstdint>

namespace fl {
namespace mvir {

struct ConstantValue {
    enum class Kind {
        Void,
        Int,
        UInt,
        Float,
        Bool,
        String,
        Array,
        Tuple,
        Struct,
        Error
    };

    Kind kind = Kind::Void;
    union {
        int64_t iVal;
        uint64_t uVal;
        double fVal;
        bool bVal;
    };
    std::string sVal; // For string literals
    std::vector<ConstantValue> elements; // For Array, Tuple
    std::unordered_map<std::string, ConstantValue> fields; // For Struct

    ConstantValue() : kind(Kind::Void), iVal(0) {}
    
    static ConstantValue makeInt(int64_t v) { ConstantValue val; val.kind = Kind::Int; val.iVal = v; return val; }
    static ConstantValue makeUInt(uint64_t v) { ConstantValue val; val.kind = Kind::UInt; val.uVal = v; return val; }
    static ConstantValue makeFloat(double v) { ConstantValue val; val.kind = Kind::Float; val.fVal = v; return val; }
    static ConstantValue makeBool(bool v) { ConstantValue val; val.kind = Kind::Bool; val.bVal = v; return val; }
    static ConstantValue makeString(const std::string& v) { ConstantValue val; val.kind = Kind::String; val.sVal = v; return val; }
    static ConstantValue makeArray(const std::vector<ConstantValue>& elems) { ConstantValue val; val.kind = Kind::Array; val.elements = elems; return val; }
    static ConstantValue makeTuple(const std::vector<ConstantValue>& elems) { ConstantValue val; val.kind = Kind::Tuple; val.elements = elems; return val; }
    static ConstantValue makeStruct(const std::unordered_map<std::string, ConstantValue>& flds) { ConstantValue val; val.kind = Kind::Struct; val.fields = flds; return val; }
    static ConstantValue makeVoid() { return ConstantValue(); }
    static ConstantValue makeError() { ConstantValue val; val.kind = Kind::Error; return val; }

    bool isError() const { return kind == Kind::Error; }

    std::string toString() const {
        switch (kind) {
            case Kind::Int: return std::to_string(iVal);
            case Kind::UInt: return std::to_string(uVal);
            case Kind::Float: return std::to_string(fVal);
            case Kind::Bool: return bVal ? "true" : "false";
            case Kind::String: return "\"" + sVal + "\""; // Very simple escaping
            case Kind::Array:
            case Kind::Tuple:
            case Kind::Struct: return "<structured>"; // TODO: Implement if needed
            case Kind::Void: return "void";
            case Kind::Error: return "<error>";
        }
        return "<unknown>";
    }
};

} // namespace mvir
} // namespace fl
