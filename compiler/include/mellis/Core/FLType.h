// =============================================================================
// mellis/Core/FLType.h
//
// Semantic Type System for mellis (Phase 4).
// Represents the actual types assigned to expressions and symbols during Type Checking.
//
// Design:
//   - Pure virtual class hierarchy (`Type` base class).
//   - Used by TypeChecker for inference and validation.
//   - Distinct from AST TypeNodes (which represent raw syntax annotations).
// =============================================================================

#pragma once
#include <string>
#include <vector>
#include <unordered_map>
#include <unordered_set>
#include <memory>
#include <sstream>
#include <iostream>
#include <algorithm>
#include "mellis/Core/Types.h"
#include "mellis/AST/TypeNode.h" // For BuiltinKind

namespace fl {

// Semantic Type Kinds
enum class TypeKind : uint8_t {
    Primitive,
    Struct,
    Enum,
    Function,
    Pointer,
    Reference,
    Array,
    Slice,
    Tuple,
    Generic,
    TypeParameter,
    GenericParam,
    Trait,
    TraitObject,
    InferenceVar,
    Never,
    Void,
    Closure,
    Future,
    Unknown,
    Error,
    AssociatedProjection, // <T as Trait>::AssocName
    TypeAlias,            // Semantic alias
    Lifetime              // Represents a lifetime argument in a generic parameter list
};

// =============================================================================
// Lifetime Semantics
// =============================================================================
using LifetimeId = uint32_t;
constexpr LifetimeId kInvalidLifetimeId = 0xFFFFFFFF;

enum class LifetimeKind : uint8_t {
    Named,        // 'a
    Anonymous,    // '_
    Static,       // 'static
    Inference,    // inference variable during type checking
    Placeholder   // Used for higher-ranked trait bounds
};

struct Lifetime {
    LifetimeId id = kInvalidLifetimeId;
    LifetimeKind kind = LifetimeKind::Named;
    std::string name; // "a", "static", etc.
    
    bool operator==(const Lifetime& other) const {
        if (kind != other.kind) return false;
        if (kind == LifetimeKind::Named) return id == other.id;
        return true; // Simplistic for now
    }
    bool operator!=(const Lifetime& other) const { return !(*this == other); }
};


enum class LifetimeRelation : uint8_t {
    Outlives,  // 'a : 'b
    Equal,     // 'a == 'b
};

struct LifetimeConstraint {
    Lifetime left;
    LifetimeRelation relation;
    Lifetime right;
};

// Base class for all Semantic Types
class Type {
public:
    virtual ~Type() = default;
    
    virtual TypeKind getKind() const = 0;
    virtual std::string toString() const = 0;
    
    // Transparent unwrap for TypeAlias
    virtual const Type* unwrapAlias() const { return this; }
    
    // Exact equality check. For subtyping/coercion, use a separate TypeChecker utility.
    virtual bool equals(const Type* other) const = 0;

    // DST (Dynamically Sized Type) check
    virtual bool isSized() const { return true; } // By default, most types are sized
    
    // Auto-Drop check
    virtual bool needsDrop() const { return false; }

    // Move semantics: is this type implicitly copyable? (e.g. primitives, immutable refs)
    virtual bool isCopy() const { return false; }
};

// -----------------------------------------------------------------------------
class LifetimeType : public Type {
public:
    Lifetime lt;

    LifetimeType(Lifetime lt) : lt(lt) {}

    TypeKind getKind() const override { return TypeKind::Lifetime; }
    
    std::string toString() const override {
        if (lt.kind == LifetimeKind::Static) return "'static";
        if (lt.kind == LifetimeKind::Anonymous) return "'_";
        return "'" + lt.name;
    }
    
    bool equals(const Type* other) const override {
        if (!other || other->getKind() != TypeKind::Lifetime) return false;
        return lt == static_cast<const LifetimeType*>(other)->lt;
    }
};

// -----------------------------------------------------------------------------
// Primitive Type (int_32, bool, float_64, etc.)
// -----------------------------------------------------------------------------
class PrimitiveType : public Type {
public:
    BuiltinKind builtinKind;
    
    explicit PrimitiveType(BuiltinKind k) : builtinKind(k) {}
    
    TypeKind getKind() const override { return TypeKind::Primitive; }
    
    std::string toString() const override {
        switch (builtinKind) {
            case BuiltinKind::Void: return "void";
            case BuiltinKind::Bool: return "bool";
            case BuiltinKind::Char: return "char";
            case BuiltinKind::Str: return "str";
            case BuiltinKind::I4: return "int_4";
            case BuiltinKind::I8: return "int_8";
            case BuiltinKind::I16: return "int_16";
            case BuiltinKind::I32: return "int_32";
            case BuiltinKind::I64: return "int_64";
            case BuiltinKind::I128: return "int_128";
            case BuiltinKind::U4: return "uint_4";
            case BuiltinKind::U8: return "uint_8";
            case BuiltinKind::U16: return "uint_16";
            case BuiltinKind::U32: return "uint_32";
            case BuiltinKind::U64: return "uint_64";
            case BuiltinKind::U128: return "uint_128";
            case BuiltinKind::F32: return "float_32";
            case BuiltinKind::F64: return "float_64";
        }
        return "?";
    }
    
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const PrimitiveType*>(other)) {
            return builtinKind == o->builtinKind;
        }
        return false;
    }
    
    bool isCopy() const override { return true; }
};

// -----------------------------------------------------------------------------
// Error Type (Used for Error Recovery)
// -----------------------------------------------------------------------------
class ErrorType : public Type {
public:
    TypeKind getKind() const override { return TypeKind::Error; }
    std::string toString() const override { return "{error}"; }
    bool equals(const Type* other) const override {
        return other->getKind() == TypeKind::Error;
    }
};

// -----------------------------------------------------------------------------
// Unknown Type (Used as placeholder before inference)
// -----------------------------------------------------------------------------
class UnknownType : public Type {
public:
    TypeKind getKind() const override { return TypeKind::Unknown; }
    std::string toString() const override { return "unknown"; }
    bool equals(const Type* other) const override {
        return other->getKind() == TypeKind::Unknown;
    }
};

// -----------------------------------------------------------------------------
// Future Type (Async/Await)
// -----------------------------------------------------------------------------
class FutureType : public Type {
public:
    const Type* innerType;

    FutureType(const Type* inner) : innerType(inner) {}

    TypeKind getKind() const override { return TypeKind::Future; }

    std::string toString() const override {
        return "Future<" + innerType->toString() + ">";
    }

    bool equals(const Type* other) const override {
        if (auto* f = dynamic_cast<const FutureType*>(other)) {
            return innerType->equals(f->innerType);
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Void Type
// -----------------------------------------------------------------------------
class VoidType : public Type {
public:
    TypeKind getKind() const override { return TypeKind::Void; }
    std::string toString() const override { return "void"; }
    bool equals(const Type* other) const override {
        return other->getKind() == TypeKind::Void;
    }
};

// -----------------------------------------------------------------------------
// Never Type (!)
// -----------------------------------------------------------------------------
class NeverType : public Type {
public:
    TypeKind getKind() const override { return TypeKind::Never; }
    std::string toString() const override { return "!"; }
    bool equals(const Type* other) const override {
        return other->getKind() == TypeKind::Never;
    }
};

// -----------------------------------------------------------------------------
// Struct Type (Refers to a struct declaration via SymbolID)
// -----------------------------------------------------------------------------
class StructType : public Type {
public:
    SymbolID structSymbolId;
    std::vector<const Type*> genericArgs;
    std::unordered_map<std::string, size_t> fieldIndices;
    std::vector<const Type*> fieldTypes;
    bool hasDrop;
    SymbolID originalTemplateId = kInvalidSymbolID;
    std::vector<const Type*> specializedArgs;
    
    explicit StructType(SymbolID id, std::vector<const Type*> args = {}, bool hasDrop = false) 
        : structSymbolId(id), genericArgs(std::move(args)), hasDrop(hasDrop) {}
    
    TypeKind getKind() const override { return TypeKind::Struct; }
    bool needsDrop() const override { return hasDrop; }
    std::string toString() const override {
        std::string s = "Struct<" + std::to_string(structSymbolId);
        if (!genericArgs.empty()) {
            s += "<";
            for (size_t i = 0; i < genericArgs.size(); ++i) {
                s += genericArgs[i]->toString();
                if (i + 1 < genericArgs.size()) s += ", ";
            }
            s += ">";
        }
        s += ">";
        return s;
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const StructType*>(other)) {
            if (structSymbolId != o->structSymbolId) return false;
            if (genericArgs.size() != o->genericArgs.size()) return false;
            for (size_t i = 0; i < genericArgs.size(); ++i) {
                if (!genericArgs[i]->equals(o->genericArgs[i])) return false;
            }
            return true;
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Enum Type (Refers to an enum declaration via SymbolID)
// -----------------------------------------------------------------------------
class EnumType : public Type {
public:
    SymbolID enumSymbolId;
    std::vector<const Type*> genericArgs;
    bool hasDrop;
    SymbolID originalTemplateId = kInvalidSymbolID;
    std::vector<const Type*> specializedArgs;
    
    explicit EnumType(SymbolID id, std::vector<const Type*> args = {}, bool hasDrop = false) 
        : enumSymbolId(id), genericArgs(std::move(args)), hasDrop(hasDrop) {}
    
    TypeKind getKind() const override { return TypeKind::Enum; }
    bool needsDrop() const override { return hasDrop; }
    std::string toString() const override {
        std::string s = "Enum<" + std::to_string(enumSymbolId);
        if (!genericArgs.empty()) {
            s += "<";
            for (size_t i = 0; i < genericArgs.size(); ++i) {
                s += genericArgs[i]->toString();
                if (i + 1 < genericArgs.size()) s += ", ";
            }
            s += ">";
        }
        s += ">";
        return s;
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const EnumType*>(other)) {
            if (enumSymbolId != o->enumSymbolId) return false;
            if (genericArgs.size() != o->genericArgs.size()) return false;
            for (size_t i = 0; i < genericArgs.size(); ++i) {
                if (!genericArgs[i]->equals(o->genericArgs[i])) return false;
            }
            return true;
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Function Type (param types -> return type)
// -----------------------------------------------------------------------------

enum class EscapeBehavior {
    NonEscaping, // The parameter (e.g., closure) does not escape the function scope
    Escaping     // The parameter may be stored or returned, outliving the function call
};

class FunctionType : public Type {
public:
    std::vector<std::string> paramNames;
    std::vector<const Type*> paramTypes;
    std::vector<EscapeBehavior> paramEscapeBehaviors; // Same length as paramTypes
    const Type* returnType;
    bool isCallSite;
    bool isVariadic;
    bool isUnsafe;
    
    FunctionType(std::vector<std::string> names, std::vector<const Type*> params, const Type* ret, bool callSite = false, bool variadic = false, bool unsafe = false)
        : paramNames(std::move(names)), paramTypes(std::move(params)), paramEscapeBehaviors(params.size(), EscapeBehavior::Escaping), returnType(ret), isCallSite(callSite), isVariadic(variadic), isUnsafe(unsafe) {}

        
    TypeKind getKind() const override { return TypeKind::Function; }
    std::string toString() const override {
        std::string s = isUnsafe ? "unsafe fn(" : "fn(";
        for (size_t i = 0; i < paramTypes.size(); ++i) {
            if (i < paramNames.size() && !paramNames[i].empty()) {
                s += paramNames[i] + ": ";
            }
            s += paramTypes[i]->toString();
            if (i + 1 < paramTypes.size() || isVariadic) s += ", ";
        }
        if (isVariadic) s += "...";
        s += ") -> " + returnType->toString();
        return s;
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const FunctionType*>(other)) {
            if (isUnsafe != o->isUnsafe) return false;
            if (isVariadic != o->isVariadic) return false;
            if (paramTypes.size() != o->paramTypes.size()) return false;
            for (size_t i = 0; i < paramTypes.size(); ++i) {
                if (!paramTypes[i]->equals(o->paramTypes[i])) return false;
            }
            return returnType->equals(o->returnType);
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Closure Type (Anonymous struct for lambdas)
// -----------------------------------------------------------------------------
enum class CaptureMode : uint8_t {
    Copy,
    Borrow,
    BorrowMut,
    Move
};

enum class CaptureSource : uint8_t {
    Direct,
    ParentCapture
};

struct CaptureInfo {
    SymbolID      symbolId;
    CaptureMode   mode;
    CaptureSource source;
    const Type*   sourceType = nullptr; // Original type of the captured variable
    const Type*   envType    = nullptr; // Type stored in the closure environment (T, &'a T, &'a rw T)
};

enum class ClosureStorageKind { Stack, Heap, None };

class ClosureType : public Type {
public:
    SymbolID structSymbolId; // ID of the anonymous struct
    SymbolID generatedFuncId;
    const FunctionType* signature;
    std::vector<CaptureInfo> captures;
    
    // Memory layout matches a StructType:
    std::vector<const Type*> fieldTypes; // First field is func ptr, remaining are captured vars
    
    ClosureType(SymbolID structId, SymbolID funcId, const FunctionType* sig, std::vector<CaptureInfo> caps)
        : structSymbolId(structId), generatedFuncId(funcId), signature(sig), captures(std::move(caps)) {}

        
    TypeKind getKind() const override { return TypeKind::Closure; }
    
    bool needsDrop() const override {
        for (const auto& cap : captures) {
            if (cap.envType && cap.envType->needsDrop()) {
                return true;
            }
        }
        return false;
    }
    
    std::string toString() const override {
        return "[closure@" + std::to_string(structSymbolId) + "]";
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const ClosureType*>(other)) {
            return structSymbolId == o->structSymbolId;
        }
        return false;
    }
    
    // Default isCopy to true if all captures are copyable (Stack closures).
    // Note: BorrowAnalyzer/MoveChecker will OVERRIDE this for Heap closures (which are Move-Only).
    bool isCopy() const override {
        for (const auto& cap : captures) {
            if (cap.envType && !cap.envType->isCopy()) return false;
        }
        return true;
    }
};

// -----------------------------------------------------------------------------
// Pointer Type (*T)
// -----------------------------------------------------------------------------
class PointerType : public Type {
public:
    const Type* pointee;
    bool isMutable;
    
    PointerType(const Type* p, bool mut) : pointee(p), isMutable(mut) {}
    
    TypeKind getKind() const override { return TypeKind::Pointer; }
    std::string toString() const override {
        return (isMutable ? "*mut " : "*const ") + pointee->toString();
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const PointerType*>(other)) {
            return isMutable == o->isMutable && pointee->equals(o->pointee);
        }
        return false;
    }
    
    bool isCopy() const override { return true; } // Raw pointers are Copy
};

// -----------------------------------------------------------------------------
// Reference Type (&T)
// -----------------------------------------------------------------------------
class ReferenceType : public Type {
public:
    const Type* pointee;
    bool isMutable;
    Lifetime lifetime;
    
    ReferenceType(const Type* p, bool mut, Lifetime lt = {}) : pointee(p), isMutable(mut), lifetime(std::move(lt)) {}
    
    TypeKind getKind() const override { return TypeKind::Reference; }
    std::string toString() const override {
        std::string s = "&";
        if (lifetime.kind != LifetimeKind::Anonymous) {
            s += "'" + lifetime.name + " ";
        }
        if (isMutable) s += "mut ";
        return s + pointee->toString();
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const ReferenceType*>(other)) {
            return isMutable == o->isMutable && lifetime == o->lifetime && pointee->equals(o->pointee);
        }
        return false;
    }
    
    bool isCopy() const override { return !isMutable; } // &T is Copy, &mut T is Move
};

// -----------------------------------------------------------------------------
// Trait Type
// -----------------------------------------------------------------------------
class TraitType : public Type {
public:
    SymbolID traitId;
    std::unordered_map<std::string, const Type*> associatedBindings;
    
    explicit TraitType(SymbolID id) : traitId(id) {}
    TraitType(SymbolID id, std::unordered_map<std::string, const Type*> bindings) 
        : traitId(id), associatedBindings(std::move(bindings)) {}
    
    TypeKind getKind() const override { return TypeKind::Trait; }
    std::string toString() const override {
        std::string s = "Trait<" + std::to_string(traitId);
        if (!associatedBindings.empty()) {
            s += " {";
            bool first = true;
            for (const auto& kv : associatedBindings) {
                if (!first) s += ", ";
                s += kv.first + " = " + kv.second->toString();
                first = false;
            }
            s += "}";
        }
        s += ">";
        return s;
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const TraitType*>(other)) {
            if (traitId != o->traitId) return false;
            if (associatedBindings.size() != o->associatedBindings.size()) return false;
            for (const auto& kv : associatedBindings) {
                auto it = o->associatedBindings.find(kv.first);
                if (it == o->associatedBindings.end()) return false;
                if (!kv.second->equals(it->second)) return false;
            }
            return true;
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Trait Object Type (dyn Trait)
// -----------------------------------------------------------------------------
class TraitObjectType : public Type {
public:
    std::vector<SymbolID> traitIds;
    Lifetime lifetime;
    
    explicit TraitObjectType(std::vector<SymbolID> ids, Lifetime lt = {}) 
        : traitIds(std::move(ids)), lifetime(std::move(lt)) {}
    
    TypeKind getKind() const override { return TypeKind::TraitObject; }
    std::string toString() const override {
        std::string s = "dyn ";
        for (size_t i = 0; i < traitIds.size(); ++i) {
            s += "Trait<" + std::to_string(traitIds[i]) + ">";
            if (i + 1 < traitIds.size()) s += " + ";
        }
        if (lifetime.kind != LifetimeKind::Anonymous && lifetime.kind != LifetimeKind::Static) { // Static/Anonymous handle differently or show always? Let's just show if named/static
             if (lifetime.kind == LifetimeKind::Static) s += " + 'static";
             else if (lifetime.kind == LifetimeKind::Named) s += " + '" + lifetime.name;
        }
        return s;
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const TraitObjectType*>(other)) {
            if (traitIds.size() != o->traitIds.size()) return false;
            for (size_t i = 0; i < traitIds.size(); ++i) {
                if (traitIds[i] != o->traitIds[i]) return false;
            }
            return lifetime == o->lifetime;
        }
        return false;
    }
    // DST constraint: dyn Trait is NOT sized
    bool isSized() const override { return false; }
};

// -----------------------------------------------------------------------------
// Array Type ([T; N])
// -----------------------------------------------------------------------------
class ArrayType : public Type {
public:
    const Type* elementType;
    size_t length;
    
    ArrayType(const Type* elem, size_t len) : elementType(elem), length(len) {}
    
    TypeKind getKind() const override { return TypeKind::Array; }
    std::string toString() const override {
        return "[" + elementType->toString() + "; " + std::to_string(length) + "]";
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const ArrayType*>(other)) {
            return length == o->length && elementType->equals(o->elementType);
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Slice Type ([T])
// -----------------------------------------------------------------------------
class SliceType : public Type {
public:
    const Type* elementType;
    
    explicit SliceType(const Type* elem) : elementType(elem) {}
    
    TypeKind getKind() const override { return TypeKind::Slice; }
    std::string toString() const override {
        return "[" + elementType->toString() + "]";
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const SliceType*>(other)) {
            return elementType->equals(o->elementType);
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Tuple Type (e.g. (i32, f32))
// -----------------------------------------------------------------------------
class TupleType : public Type {
public:
    std::vector<const Type*> elements;
    
    explicit TupleType(std::vector<const Type*> elems) : elements(std::move(elems)) {}
    
    TypeKind getKind() const override { return TypeKind::Tuple; }
    std::string toString() const override {
        std::string s = "(";
        for (size_t i = 0; i < elements.size(); ++i) {
            s += elements[i]->toString();
            if (i + 1 < elements.size()) s += ", ";
        }
        s += ")";
        return s;
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const TupleType*>(other)) {
            if (elements.size() != o->elements.size()) return false;
            for (size_t i = 0; i < elements.size(); ++i) {
                if (!elements[i]->equals(o->elements[i])) return false;
            }
            return true;
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Inference Variable Type (e.g. ?T)
// -----------------------------------------------------------------------------
class InferenceVarType : public Type {
public:
    uint32_t varId;
    
    explicit InferenceVarType(uint32_t id) : varId(id) {}
    
    TypeKind getKind() const override { return TypeKind::InferenceVar; }
    std::string toString() const override {
        return "?T" + std::to_string(varId);
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const InferenceVarType*>(other)) {
            return varId == o->varId;
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Generic Parameter Type (T, U, etc.)
// -----------------------------------------------------------------------------
class GenericParamType : public Type {
public:
    SymbolID paramId;
    std::string name;
    
    GenericParamType(SymbolID id, std::string n) : paramId(id), name(std::move(n)) {}
    
    TypeKind getKind() const override { return TypeKind::GenericParam; }
    std::string toString() const override {
        return name + " (" + std::to_string(paramId) + ")";
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const GenericParamType*>(other)) {
            return paramId == o->paramId;
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// AssociatedTypeProjection: <T as Trait>::AssocName
// Represents a not-yet-resolved associated type projection.
// TraitSolver resolves this into a concrete type.
// E.g.: in `fn next() -> Self::Item`, Item is initially an AssociatedTypeProjection.
// -----------------------------------------------------------------------------
class AssociatedTypeProjection : public Type {
public:
    const Type* selfType;    // The self type (T in <T as Trait>)
    SymbolID traitId;        // The trait ID
    std::string assocName;   // The associated type name ("Item", "Error", etc.)

    AssociatedTypeProjection(const Type* self, SymbolID trait, std::string name)
        : selfType(self), traitId(trait), assocName(std::move(name)) {}

    TypeKind getKind() const override { return TypeKind::AssociatedProjection; }
    std::string toString() const override {
        std::string selfStr = selfType ? selfType->toString() : "?";
        return "<" + selfStr + " as Trait<" + std::to_string(traitId) + ">>::" + assocName;
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const AssociatedTypeProjection*>(other)) {
            return traitId == o->traitId && assocName == o->assocName &&
                   ((selfType == o->selfType) || (selfType && o->selfType && selfType->equals(o->selfType)));
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Type Alias Type
// Represents a semantic alias (e.g. `type UserId = uint_64;`).
// -----------------------------------------------------------------------------
class TypeAliasType : public Type {
public:
    SymbolID aliasId;
    std::string aliasName;
    std::vector<const Type*> genericArgs;
    const Type* aliasedType;
    
    TypeAliasType(SymbolID id, std::string name, std::vector<const Type*> args, const Type* aliased)
        : aliasId(id), aliasName(std::move(name)), genericArgs(std::move(args)), aliasedType(aliased) {}
        
    TypeKind getKind() const override { return TypeKind::TypeAlias; }
    
    const Type* unwrapAlias() const override {
        return aliasedType->unwrapAlias(); // recursive unwrap
    }
    
    std::string toString() const override {
        if (genericArgs.empty()) return aliasName;
        std::string s = aliasName + "<";
        for (size_t i = 0; i < genericArgs.size(); ++i) {
            s += genericArgs[i]->toString();
            if (i + 1 < genericArgs.size()) s += ", ";
        }
        s += ">";
        return s;
    }
    
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const TypeAliasType*>(other)) {
            if (aliasId == o->aliasId && genericArgs.size() == o->genericArgs.size()) {
                bool argsMatch = true;
                for (size_t i = 0; i < genericArgs.size(); ++i) {
                    if (!genericArgs[i]->equals(o->genericArgs[i])) {
                        argsMatch = false;
                        break;
                    }
                }
                if (argsMatch) return true;
            }
        }
        return aliasedType->equals(other);
    }
};

// =============================================================================
// TypeContext (Phase 4.2.1)
// Acts as an arena allocator and interner for all Semantic Types.
// Ensures that types are deduplicated (e.g. only one i32 instance) and
// simplifies type comparison to simple pointer equality (const Type* == const Type*).
// =============================================================================
// =============================================================================
// Unification Table (Type Inference)
// =============================================================================
class TypeContext;

class UnificationTable {
    std::vector<const Type*> vars;
    std::vector<uint32_t> unifiedLog; // Logs IDs that were unified for rollback
public:
    struct Snapshot {
        size_t varsSize;
        size_t logSize;
    };

    uint32_t newVar() {
        uint32_t id = vars.size();
        vars.push_back(nullptr);
        return id;
    }
    
    void unify(uint32_t id, const Type* t) {
        if (!vars[id]) {
            unifiedLog.push_back(id);
        }
        vars[id] = t;
    }
    
    const Type* resolve(uint32_t id) const {
        return vars[id]; // Might be nullptr if unbound
    }
    
    Snapshot snapshot() const {
        return {vars.size(), unifiedLog.size()};
    }

    void rollback(const Snapshot& snap) {
        // Revert unified variables
        while (unifiedLog.size() > snap.logSize) {
            vars[unifiedLog.back()] = nullptr;
            unifiedLog.pop_back();
        }
        // Remove newly created variables
        if (vars.size() > snap.varsSize) {
            vars.resize(snap.varsSize);
        }
    }

    void commit(const Snapshot& snap) {
        // Nothing to do for commit in this design, log remains for outer snapshots
    }

    // Deep resolve a type, replacing InferenceVarTypes with their unified type
    const Type* deepResolve(const Type* t, TypeContext& ctx) const;
};

class TypeContext {
    std::vector<std::unique_ptr<Type>> arena_;
    
    // Cached primitives
    const Type* prims_[(size_t)BuiltinKind::Void + 1] = {nullptr};
    const Type* neverType_ = nullptr;
    const Type* voidType_ = nullptr;
    const Type* unknownType_ = nullptr;
    const Type* errorType_ = nullptr;

public:
    UnificationTable unificationTable;
    std::vector<LifetimeConstraint> lifetimeConstraints;

    ~TypeContext() {
        extern bool g_quiet; if (!g_quiet) std::cerr << "[INSTRUMENT] TypeContext Destroyed\n";
    }

    TypeContext() {
        extern bool g_quiet; if (!g_quiet) std::cerr << "[INSTRUMENT] TypeContext Created\n";
        // Pre-allocate singletons
        neverType_ = create<NeverType>();
        voidType_ = create<VoidType>();
        unknownType_ = create<UnknownType>();
        errorType_ = create<ErrorType>();
        
        // Primitives
        for (int i = 0; i <= (int)BuiltinKind::Void; ++i) {
            prims_[i] = create<PrimitiveType>(static_cast<BuiltinKind>(i));
        }
    }
    
    // Core allocation function
    template <typename T, typename... Args>
    const T* create(Args&&... args) {
        auto ptr = std::make_unique<T>(std::forward<Args>(args)...);
        const T* raw = ptr.get();
        arena_.push_back(std::move(ptr));
        return raw;
    }

    // Singleton accessors
    const Type* getPrimitive(BuiltinKind k) const { return prims_[(size_t)k]; }
    const Type* getNever() const { return neverType_; }
    const Type* getVoid() const { return voidType_; }
    const Type* getUnknown() const { return unknownType_; }
    const Type* getError() const { return errorType_; }

    uint32_t newVar() { return unificationTable.newVar(); }

    const InferenceVarType* getInferenceVar(uint32_t id) {
        for (auto& t : arena_) {
            if (auto* inf = dynamic_cast<InferenceVarType*>(t.get())) {
                if (inf->varId == id) return inf;
            }
        }
        return create<InferenceVarType>(id);
    }

    // Deduplication for Pointers / References
    // (A fully optimized interner would use a hash map, but simple iteration is fine for MVP)
    const PointerType* getPointerType(const Type* pointee, bool isMutable) {
        for (auto& t : arena_) {
            if (auto* p = dynamic_cast<PointerType*>(t.get())) {
                if (p->pointee == pointee && p->isMutable == isMutable) return p;
            }
        }
        return create<PointerType>(pointee, isMutable);
    }
    
    const ReferenceType* getReferenceType(const Type* pointee, bool isMutable, Lifetime lt = {}) {
        for (auto& t : arena_) {
            if (auto* p = dynamic_cast<ReferenceType*>(t.get())) {
                if (p->pointee == pointee && p->isMutable == isMutable && p->lifetime == lt) return p;
            }
        }
        return create<ReferenceType>(pointee, isMutable, std::move(lt));
    }
    
    const TupleType* getTupleType(const std::vector<const Type*>& elements) {
        // Simple iteration for MVP
        for (auto& t : arena_) {
            if (auto* p = dynamic_cast<TupleType*>(t.get())) {
                if (p->elements.size() == elements.size()) {
                    bool match = true;
                    for (size_t i = 0; i < elements.size(); ++i) {
                        if (p->elements[i] != elements[i]) { match = false; break; }
                    }
                    if (match) return p;
                }
            }
        }
        return create<TupleType>(elements);
    }

    
    const Type* substitute(const Type* t, const std::unordered_map<SymbolID, const Type*>& mapping, const std::unordered_map<std::string, const Type*>& nameMapping = {}) {
        if (!t) return nullptr;
        if (auto* gp = dynamic_cast<const GenericParamType*>(t)) {
            auto it = mapping.find(gp->paramId);
            std::cerr << "[DEBUG substitute] GenericParam id=" << gp->paramId << " name='" << gp->name << "' found=" << (it != mapping.end()) << "\n";
            if (it != mapping.end()) return it->second;
            auto nameIt = nameMapping.find(gp->name);
            if (nameIt != nameMapping.end()) {
                std::cerr << "[DEBUG substitute] GenericParam name='" << gp->name << "' found by name!\n";
                return nameIt->second;
            }
            return t;
        }
        if (auto* ptr = dynamic_cast<const PointerType*>(t)) {
            return getPointerType(substitute(ptr->pointee, mapping, nameMapping), ptr->isMutable);
        }
        if (auto* ref = dynamic_cast<const ReferenceType*>(t)) {
            return getReferenceType(substitute(ref->pointee, mapping, nameMapping), ref->isMutable, ref->lifetime);
        }
        if (auto* proj = dynamic_cast<const AssociatedTypeProjection*>(t)) {
            const Type* newSelf = substitute(proj->selfType, mapping, nameMapping);
            if (newSelf != proj->selfType) {
                return getAssociatedProjection(newSelf, proj->traitId, proj->assocName);
            }
            return t;
        }
        if (auto* st = dynamic_cast<const StructType*>(t)) {
            if (st->genericArgs.empty()) return t;
            std::vector<const Type*> newArgs;
            for (auto* arg : st->genericArgs) newArgs.push_back(substitute(arg, mapping, nameMapping));
            return getStructType(st->structSymbolId, std::move(newArgs));
        }
        if (auto* et = dynamic_cast<const EnumType*>(t)) {
            if (et->genericArgs.empty()) return t;
            std::vector<const Type*> newArgs;
            for (auto* arg : et->genericArgs) newArgs.push_back(substitute(arg, mapping, nameMapping));
            return getEnumType(et->enumSymbolId, std::move(newArgs));
        }
        if (auto* ft = dynamic_cast<const FunctionType*>(t)) {
            std::vector<const Type*> newParams;
            for (auto* p : ft->paramTypes) newParams.push_back(substitute(p, mapping, nameMapping));
            return getFunctionType(ft->paramNames, std::move(newParams), substitute(ft->returnType, mapping, nameMapping), ft->isCallSite, ft->isVariadic, ft->isUnsafe);
        }
        if (auto* at = dynamic_cast<const ArrayType*>(t)) {
            return getArrayType(substitute(at->elementType, mapping, nameMapping), at->length);
        }
        if (auto* sl = dynamic_cast<const SliceType*>(t)) {
            return getSliceType(substitute(sl->elementType, mapping, nameMapping));
        }
        return t; // primitive, unknown, void, never, error
    }

    const StructType* getStructType(SymbolID id, std::vector<const Type*> args = {}) {
        for (auto& t : arena_) {
            if (auto* s = dynamic_cast<StructType*>(t.get())) {
                if (s->structSymbolId == id && s->genericArgs == args) return s;
            }
        }
        return create<StructType>(id, std::move(args));
    }

    const EnumType* getEnumType(SymbolID id, std::vector<const Type*> args = {}) {
        for (auto& t : arena_) {
            if (auto* e = dynamic_cast<EnumType*>(t.get())) {
                if (e->enumSymbolId == id && e->genericArgs == args) return e;
            }
        }
        return create<EnumType>(id, std::move(args));
    }
    
    const TraitType* getTraitType(SymbolID id, std::unordered_map<std::string, const Type*> bindings = {}) {
        for (auto& t : arena_) {
            if (auto* tr = dynamic_cast<TraitType*>(t.get())) {
                if (tr->traitId == id) {
                    bool match = true;
                    if (tr->associatedBindings.size() != bindings.size()) match = false;
                    else {
                        for (const auto& kv : bindings) {
                            auto it = tr->associatedBindings.find(kv.first);
                            if (it == tr->associatedBindings.end() || !kv.second->equals(it->second)) {
                                match = false;
                                break;
                            }
                        }
                    }
                    if (match) return tr;
                }
            }
        }
        return create<TraitType>(id, std::move(bindings));
    }
    
    const TraitObjectType* getTraitObjectType(std::vector<SymbolID> ids, Lifetime lt = {}) {
        std::sort(ids.begin(), ids.end());
        for (auto& t : arena_) {
            if (auto* tr = dynamic_cast<TraitObjectType*>(t.get())) {
                if (tr->traitIds.size() == ids.size() && tr->lifetime == lt) {
                    bool match = true;
                    for (size_t i = 0; i < ids.size(); ++i) {
                        if (tr->traitIds[i] != ids[i]) { match = false; break; }
                    }
                    if (match) return tr;
                }
            }
        }
        return create<TraitObjectType>(std::move(ids), std::move(lt));
    }
    
    const TypeAliasType* getTypeAliasType(SymbolID aliasId, std::string aliasName, std::vector<const Type*> genericArgs, const Type* aliasedType) {
        for (auto& t : arena_) {
            if (auto* ta = dynamic_cast<TypeAliasType*>(t.get())) {
                if (ta->aliasId == aliasId && ta->genericArgs.size() == genericArgs.size()) {
                    bool match = true;
                    for (size_t i = 0; i < genericArgs.size(); ++i) {
                        if (!ta->genericArgs[i]->equals(genericArgs[i])) { match = false; break; }
                    }
                    if (match) return ta;
                }
            }
        }
        return create<TypeAliasType>(aliasId, std::move(aliasName), std::move(genericArgs), aliasedType);
    }

    const FunctionType* getFunctionType(std::vector<std::string> paramNames, std::vector<const Type*> paramTypes, const Type* returnType, bool isCallSite = false, bool isVariadic = false, bool isUnsafe = false) {
        // Full deduplication skipped for simplicity, just create
        return create<FunctionType>(std::move(paramNames), std::move(paramTypes), returnType, isCallSite, isVariadic, isUnsafe);
    }
    const FunctionType* getFunctionType(std::vector<const Type*> paramTypes, const Type* returnType, bool isCallSite = false, bool isVariadic = false, bool isUnsafe = false) {
        std::vector<std::string> emptyNames(paramTypes.size(), "");
        return getFunctionType(std::move(emptyNames), std::move(paramTypes), returnType, isCallSite, isVariadic, isUnsafe);
    }
    
    const ArrayType* getArrayType(const Type* elementType, size_t length) {
        return create<ArrayType>(elementType, length);
    }
    
    const SliceType* getSliceType(const Type* elementType) {
        return create<SliceType>(elementType);
    }
    
    const GenericParamType* getGenericParamType(SymbolID id, std::string_view name) {
        for (auto& t : arena_) {
            if (auto* gp = dynamic_cast<GenericParamType*>(t.get())) {
                if (gp->paramId == id) return gp;
            }
        }
        return create<GenericParamType>(id, std::string(name));
    }
    
    const Type* getLifetimeType(const Lifetime& lt) {
        return create<LifetimeType>(lt);
    }

    const AssociatedTypeProjection* getAssociatedProjection(const Type* selfType, SymbolID traitId, const std::string& assocName) {
        for (auto& t : arena_) {
            if (auto* proj = dynamic_cast<AssociatedTypeProjection*>(t.get())) {
                if (proj->traitId == traitId && proj->assocName == assocName &&
                    proj->selfType == selfType) {
                    return proj;
                }
            }
        }
        return create<AssociatedTypeProjection>(selfType, traitId, assocName);
    }
};


inline const Type* UnificationTable::deepResolve(const Type* t, TypeContext& ctx) const {
    if (!t) return nullptr;
    if (auto* inf = dynamic_cast<const InferenceVarType*>(t)) {
        if (const Type* res = resolve(inf->varId)) {
            return deepResolve(res, ctx);
        }
        return t; // unbound
    }
    if (auto* ref = dynamic_cast<const ReferenceType*>(t)) {
        const Type* resolvedInner = deepResolve(ref->pointee, ctx);
        if (resolvedInner != ref->pointee) return ctx.getReferenceType(resolvedInner, ref->isMutable);
        return t;
    }
    if (auto* ptr = dynamic_cast<const PointerType*>(t)) {
        const Type* resolvedInner = deepResolve(ptr->pointee, ctx);
        if (resolvedInner != ptr->pointee) return ctx.getPointerType(resolvedInner, ptr->isMutable);
        return t;
    }
    if (auto* tup = dynamic_cast<const TupleType*>(t)) {
        bool changed = false;
        std::vector<const Type*> resolvedElems;
        for (auto* elem : tup->elements) {
            const Type* resolvedElem = deepResolve(elem, ctx);
            if (resolvedElem != elem) changed = true;
            resolvedElems.push_back(resolvedElem);
        }
        if (changed) return ctx.getTupleType(resolvedElems);
        return t;
    }
    return t; 
}

} // namespace fl
