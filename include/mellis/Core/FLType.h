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
#include <unordered_map>
#include <string>
#include <vector>
#include <memory>
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
    Unknown
};

// Base class for all Semantic Types
class Type {
public:
    virtual ~Type() = default;
    
    virtual TypeKind getKind() const = 0;
    virtual std::string toString() const = 0;
    
    // Exact equality check. For subtyping/coercion, use a separate TypeChecker utility.
    virtual bool equals(const Type* other) const = 0;

    // DST (Dynamically Sized Type) check
    virtual bool isSized() const { return true; } // By default, most types are sized
    
    // Auto-Drop check
    virtual bool needsDrop() const { return false; }
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
class FunctionType : public Type {
public:
    std::vector<std::string> paramNames;
    std::vector<const Type*> paramTypes;
    const Type* returnType;
    bool isCallSite;
    bool isVariadic;
    
    FunctionType(std::vector<std::string> names, std::vector<const Type*> params, const Type* ret, bool callSite = false, bool variadic = false)
        : paramNames(std::move(names)), paramTypes(std::move(params)), returnType(ret), isCallSite(callSite), isVariadic(variadic) {}
        
    TypeKind getKind() const override { return TypeKind::Function; }
    std::string toString() const override {
        std::string s = "fn(";
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
class ClosureType : public Type {
public:
    SymbolID structSymbolId; // ID of the anonymous struct
    SymbolID generatedFuncId;
    const FunctionType* signature;
    std::vector<SymbolID> capturedSymbols;
    
    // Memory layout matches a StructType:
    std::vector<const Type*> fieldTypes; // First field is func ptr, remaining are captured vars
    
    ClosureType(SymbolID structId, SymbolID funcId, const FunctionType* sig, std::vector<SymbolID> captures)
        : structSymbolId(structId), generatedFuncId(funcId), signature(sig), capturedSymbols(std::move(captures)) {}
        
    TypeKind getKind() const override { return TypeKind::Closure; }
    std::string toString() const override {
        return "[closure@" + std::to_string(structSymbolId) + "]";
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const ClosureType*>(other)) {
            return structSymbolId == o->structSymbolId;
        }
        return false;
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
};

// -----------------------------------------------------------------------------
// Reference Type (&T)
// -----------------------------------------------------------------------------
class ReferenceType : public Type {
public:
    const Type* pointee;
    bool isMutable;
    
    ReferenceType(const Type* p, bool mut) : pointee(p), isMutable(mut) {}
    
    TypeKind getKind() const override { return TypeKind::Reference; }
    std::string toString() const override {
        return (isMutable ? "&mut " : "&") + pointee->toString();
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const ReferenceType*>(other)) {
            return isMutable == o->isMutable && pointee->equals(o->pointee);
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Trait Type
// -----------------------------------------------------------------------------
class TraitType : public Type {
public:
    SymbolID traitId;
    
    explicit TraitType(SymbolID id) : traitId(id) {}
    
    TypeKind getKind() const override { return TypeKind::Trait; }
    std::string toString() const override {
        return "Trait<" + std::to_string(traitId) + ">";
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const TraitType*>(other)) {
            return traitId == o->traitId;
        }
        return false;
    }
};

// -----------------------------------------------------------------------------
// Trait Object Type (dyn Trait)
// -----------------------------------------------------------------------------
class TraitObjectType : public Type {
public:
    SymbolID traitId;
    
    explicit TraitObjectType(SymbolID id) : traitId(id) {}
    
    TypeKind getKind() const override { return TypeKind::TraitObject; }
    std::string toString() const override {
        return "dyn Trait<" + std::to_string(traitId) + ">";
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const TraitObjectType*>(other)) {
            return traitId == o->traitId;
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
    std::string_view name;
    
    GenericParamType(SymbolID id, std::string_view n) : paramId(id), name(n) {}
    
    TypeKind getKind() const override { return TypeKind::GenericParam; }
    std::string toString() const override {
        return std::string(name);
    }
    bool equals(const Type* other) const override {
        if (auto* o = dynamic_cast<const GenericParamType*>(other)) {
            return paramId == o->paramId;
        }
        return false;
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
public:
    uint32_t newVar() {
        uint32_t id = vars.size();
        vars.push_back(nullptr);

        return id;
    }
    
    void unify(uint32_t id, const Type* t) {
        vars[id] = t;
    }
    
    const Type* resolve(uint32_t id) const {
        return vars[id]; // Might be nullptr if unbound
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

public:
    UnificationTable unificationTable;

    TypeContext() {
        // Pre-allocate singletons
        neverType_ = create<NeverType>();
        voidType_ = create<VoidType>();
        unknownType_ = create<UnknownType>();
        
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
    
    const ReferenceType* getReferenceType(const Type* pointee, bool isMutable) {
        for (auto& t : arena_) {
            if (auto* p = dynamic_cast<ReferenceType*>(t.get())) {
                if (p->pointee == pointee && p->isMutable == isMutable) return p;
            }
        }
        return create<ReferenceType>(pointee, isMutable);
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

    
    const Type* substitute(const Type* t, const std::unordered_map<SymbolID, const Type*>& mapping) {
        if (!t) return nullptr;
        if (auto* gp = dynamic_cast<const GenericParamType*>(t)) {
            auto it = mapping.find(gp->paramId);
            if (it != mapping.end()) return it->second;
            return t;
        }
        if (auto* ptr = dynamic_cast<const PointerType*>(t)) {
            return getPointerType(substitute(ptr->pointee, mapping), ptr->isMutable);
        }
        if (auto* ref = dynamic_cast<const ReferenceType*>(t)) {
            return getReferenceType(substitute(ref->pointee, mapping), ref->isMutable);
        }
        if (auto* st = dynamic_cast<const StructType*>(t)) {
            if (st->genericArgs.empty()) return t;
            std::vector<const Type*> newArgs;
            for (auto* arg : st->genericArgs) newArgs.push_back(substitute(arg, mapping));
            return getStructType(st->structSymbolId, std::move(newArgs));
        }
        if (auto* et = dynamic_cast<const EnumType*>(t)) {
            if (et->genericArgs.empty()) return t;
            std::vector<const Type*> newArgs;
            for (auto* arg : et->genericArgs) newArgs.push_back(substitute(arg, mapping));
            return getEnumType(et->enumSymbolId, std::move(newArgs));
        }
        if (auto* ft = dynamic_cast<const FunctionType*>(t)) {
            std::vector<const Type*> newParams;
            for (auto* p : ft->paramTypes) newParams.push_back(substitute(p, mapping));
            return getFunctionType(ft->paramNames, std::move(newParams), substitute(ft->returnType, mapping), ft->isCallSite);
        }
        if (auto* at = dynamic_cast<const ArrayType*>(t)) {
            return getArrayType(substitute(at->elementType, mapping), at->length);
        }
        if (auto* st = dynamic_cast<const SliceType*>(t)) {
            return getSliceType(substitute(st->elementType, mapping));
        }
        return t; // primitive, unknown, void, never
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
    
    const TraitType* getTraitType(SymbolID id) {
        for (auto& t : arena_) {
            if (auto* tr = dynamic_cast<TraitType*>(t.get())) {
                if (tr->traitId == id) return tr;
            }
        }
        return create<TraitType>(id);
    }
    
    const TraitObjectType* getTraitObjectType(SymbolID id) {
        for (auto& t : arena_) {
            if (auto* tr = dynamic_cast<TraitObjectType*>(t.get())) {
                if (tr->traitId == id) return tr;
            }
        }
        return create<TraitObjectType>(id);
    }
    
    const FunctionType* getFunctionType(std::vector<std::string> paramNames, std::vector<const Type*> paramTypes, const Type* returnType, bool isCallSite = false, bool isVariadic = false) {
        // Full deduplication skipped for simplicity, just create
        return create<FunctionType>(std::move(paramNames), std::move(paramTypes), returnType, isCallSite, isVariadic);
    }
    const FunctionType* getFunctionType(std::vector<const Type*> paramTypes, const Type* returnType, bool isCallSite = false, bool isVariadic = false) {
        std::vector<std::string> emptyNames(paramTypes.size(), "");
        return getFunctionType(std::move(emptyNames), std::move(paramTypes), returnType, isCallSite, isVariadic);
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
        return create<GenericParamType>(id, name);
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
