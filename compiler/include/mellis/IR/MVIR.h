// =============================================================================
// mellis/MiddleEnd/MVIR.h
//
// Mellivora Intermediate Representation (MVIR).
//
// MVIR is a high-level representation, directly mirroring the specification
// in `docs/mvir.md`. It strictly adheres to:
//   - Non-SSA for variables (using explicit alloca/load/store)
//   - Basic Blocks ending in a single terminator
//   - Strict type annotations
//
// This layer is pure data. The AST-to-MVIR translation logic belongs
// to MVIRGenerator.
// =============================================================================

#pragma once

#include "mellis/Core/SourceLocation.h"
#include "mellis/Core/FLType.h"
#include "mellis/IR/ConstantValue.h"
#include "mellis/AST/ASTNode.h" // For Visibility
#include "mellis/Core/Intrinsic.h"
#include <string>
#include <vector>
#include <memory>
#include <variant>
#include <optional>

namespace fl {
namespace mvir {

// =============================================================================
// 1. Types
// =============================================================================

/// Convert const Type* (from AST/TypeChecker) to a string representation matching
/// `docs/mvir.md` (e.g., "i32", "bool", "void").
std::string formatType(const Type* type);

// =============================================================================
// 2. Operands
// =============================================================================

struct LocalId {
    std::string name;       ///< Human-readable debug name (e.g. "%x", "%0"). NOT used for equality.
    uint32_t symbolId   = 0; ///< Unique symbol ID from SymbolTable. 0 = synthetic/anonymous.
    uint32_t expansionId = 0; ///< Macro expansion ID (from ASTNode.expansionID). 0 = not from macro.

    std::string toString() const { return name; }

    /// Semantic equality: two locals are the same iff they refer to the same symbol
    /// AND were produced by the same macro expansion (or neither is from a macro).
    bool operator==(const LocalId& other) const {
        if (symbolId != 0 && other.symbolId != 0)
            return symbolId == other.symbolId && expansionId == other.expansionId;
        // Fallback for synthetic locals (symbolId == 0): compare by name
        return name == other.name && expansionId == other.expansionId;
    }
    bool operator!=(const LocalId& other) const { return !(*this == other); }
};


struct GlobalId {
    std::string name; // e.g., "@main"
    SymbolID symbolId = 0; // Reference back to the SymbolTable (S7.4)
    
    std::string toString() const { return name; }
    bool operator==(const GlobalId& other) const { return name == other.name; }
};

struct LabelId {
    std::string name; // e.g., "bb0"
    std::string toString() const { return name; }
    bool operator==(const LabelId& other) const { return name == other.name; }
};

struct Number {
    std::string value; // "42"
    std::string toString() const { return value; }
    bool operator==(const Number& other) const { return value == other.value; }
};

struct Boolean {
    bool value;
    std::string toString() const { return value ? "true" : "false"; }
    bool operator==(const Boolean& other) const { return value == other.value; }
};


enum class ProjectionKind {
    Field,
    TupleIndex,
    Index,
    Deref
};

struct Projection {
    ProjectionKind kind;
    size_t fieldIndex = 0;
    std::optional<LocalId> indexLocal; // For Index

    bool operator==(const Projection& other) const {
        return kind == other.kind && fieldIndex == other.fieldIndex && indexLocal == other.indexLocal;
    }
    std::string toString() const;
};

using PlaceBase = std::variant<LocalId, GlobalId>;

struct Place {
    PlaceBase base;
    std::vector<Projection> projections;

    Place() = default;
    Place(PlaceBase b) : base(std::move(b)) {}
    Place(PlaceBase b, std::vector<Projection> p) : base(std::move(b)), projections(std::move(p)) {}

    bool operator==(const Place& other) const {
        return base == other.base && projections == other.projections;
    }
    std::string toString() const;
};

using OperandBase = std::variant<Place, Number, Boolean>;

/// Operand extends the variant with implicit conversions from LocalId and GlobalId.
struct Operand : OperandBase {
    using OperandBase::OperandBase;

    /// Allow implicit construction from LocalId → Place(LocalId)
    Operand(LocalId id) : OperandBase(Place{PlaceBase{std::move(id)}}) {}

    /// Allow implicit construction from GlobalId → Place(GlobalId)
    Operand(GlobalId id) : OperandBase(Place{PlaceBase{std::move(id)}}) {}
};

const LocalId* getLocalIf(const Operand& op);
const GlobalId* getGlobalIf(const Operand& op);
const Place* getPlaceIf(const Operand& op);
Place* getPlaceIf(Operand& op);

std::string toString(const Operand& op);

// =============================================================================
// 3. Instructions
// =============================================================================


enum class Opcode : uint8_t {
    // Memory
    Local, HeapAlloc, HeapFree, Load, Store, Borrow, Cast, Drop, Sizeof, Alignof, PtrOffset, PtrDiff,
    // ALU
    Alu, Unary,
    // Extract
    Extract, TupleExtract, Tag, Variant, MakeTraitObject, MakeSlice, Call, VirtualCall, Await, BoundsCheck,
    // Terminators
    Jump, Branch, Switch, Ret, Unreachable,
    
    IntrinsicCall
};

struct Instruction {
    virtual ~Instruction() = default;
    virtual std::string toString() const = 0;
    virtual Opcode getOpcode() const = 0;
};


// ── Memory Instructions ──────────────────────────────────────────────────────

struct LocalInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Local; }
    LocalId dest;
    const Type* type;

    LocalInst(LocalId d, const Type* t) : dest(std::move(d)), type(t) {}
    std::string toString() const override;
};

struct HeapAllocInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::HeapAlloc; }
    LocalId dest;
    const Type* type;

    HeapAllocInst(LocalId d, const Type* t) : dest(std::move(d)), type(t) {}
    std::string toString() const override;
};

struct HeapFreeInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::HeapFree; }
    Operand ptr;
    const Type* type;

    HeapFreeInst(Operand p, const Type* t) : ptr(std::move(p)), type(t) {}
    std::string toString() const override;
};

struct LoadInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Load; }
    LocalId dest;
    const Type* type;
    Operand ptr;

    LoadInst(LocalId d, const Type* t, Operand p) : dest(std::move(d)), type(t), ptr(std::move(p)) {}
    std::string toString() const override;
};

struct StoreInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Store; }
    const Type* type;
    Operand value;
    Operand ptr;

    StoreInst(const Type* t, Operand v, Operand p) : type(t), value(std::move(v)), ptr(std::move(p)) {}
    std::string toString() const override;
};



struct BorrowInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Borrow; }
    LocalId dest;
    bool isMutable;
    Operand base;

    BorrowInst(LocalId d, bool m, Operand b) : dest(std::move(d)), isMutable(m), base(std::move(b)) {}
    std::string toString() const override;
};

struct CastInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Cast; }
    LocalId dest;
    Operand value;
    const Type* targetType;

    CastInst(LocalId d, Operand v, const Type* t) : dest(std::move(d)), value(std::move(v)), targetType(t) {}
    std::string toString() const override;
};

struct DropInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Drop; }
    Operand value;
    const Type* type;

    DropInst(Operand v, const Type* t) : value(std::move(v)), type(t) {}
    std::string toString() const override;
};

struct PtrOffsetInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::PtrOffset; }
    LocalId dest;
    Operand ptr;
    Operand offset;
    const Type* elementType;

    PtrOffsetInst(LocalId d, Operand p, Operand o, const Type* t) : dest(d), ptr(std::move(p)), offset(std::move(o)), elementType(t) {}
    std::string toString() const override;
};

struct PtrDiffInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::PtrDiff; }
    LocalId dest;
    Operand left;
    Operand right;
    const Type* elementType;

    PtrDiffInst(LocalId d, Operand l, Operand r, const Type* t) : dest(d), left(std::move(l)), right(std::move(r)), elementType(t) {}
    std::string toString() const override;
};

struct SizeofInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Sizeof; }
    LocalId dest;
    const Type* targetType;

    SizeofInst(LocalId d, const Type* t) : dest(std::move(d)), targetType(t) {}
    std::string toString() const override;
};

struct AlignofInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Alignof; }
    LocalId dest;
    const Type* targetType;

    AlignofInst(LocalId d, const Type* t) : dest(std::move(d)), targetType(t) {}
    std::string toString() const override;
};

struct IntrinsicCallInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::IntrinsicCall; }
    IntrinsicKind intrinsic;
    std::optional<LocalId> dest;
    std::vector<Operand> args;
    std::vector<const Type*> typeArgs;

    IntrinsicCallInst(IntrinsicKind intrinsic, std::optional<LocalId> dest, std::vector<Operand> args, std::vector<const Type*> typeArgs = {})
        : intrinsic(intrinsic), dest(std::move(dest)), args(std::move(args)), typeArgs(std::move(typeArgs)) {}
    std::string toString() const override;
};
// ── ALU Instructions ─────────────────────────────────────────────────────────

enum class AluOp {
    Add, Sub, Mul, Div,
    Eq, Ne, Lt, Le, Gt, Ge
};

std::string formatAluOp(AluOp op);

enum class UnaryOp {
    Negate,
    BitNot
};
std::string formatUnaryOp(UnaryOp op);

struct AluInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Alu; }
    LocalId dest;
    AluOp op;
    Operand left;
    Operand right;

    AluInst(LocalId d, AluOp o, Operand l, Operand r)
        : dest(std::move(d)), op(o), left(std::move(l)), right(std::move(r)) {}
    std::string toString() const override;
};

struct UnaryInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Unary; }
    LocalId dest;
    UnaryOp op;
    Operand operand;

    UnaryInst(LocalId d, UnaryOp o, Operand v)
        : dest(std::move(d)), op(o), operand(std::move(v)) {}
    std::string toString() const override;
};

// ── Call Instruction ─────────────────────────────────────────────────────────

struct ExtractInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Extract; }
    LocalId dest;
    Operand base;
    std::vector<const Type*> payloadTypes;
    size_t variantIndex;
    size_t fieldIndex;

    ExtractInst(LocalId d, Operand b, std::vector<const Type*> pTypes, size_t vIdx, size_t fIdx)
        : dest(std::move(d)), base(std::move(b)), payloadTypes(std::move(pTypes)), variantIndex(vIdx), fieldIndex(fIdx) {}
    std::string toString() const override;
};

struct TupleExtractInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::TupleExtract; }
    LocalId dest;
    Operand tuple;
    size_t index;
    const Type* elementType;

    TupleExtractInst(LocalId d, Operand t, size_t idx, const Type* elTy)
        : dest(std::move(d)), tuple(std::move(t)), index(idx), elementType(elTy) {}
    std::string toString() const override;
};

struct TagInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Tag; }
    LocalId dest;
    Operand base;

    TagInst(LocalId d, Operand b) : dest(std::move(d)), base(std::move(b)) {}
    std::string toString() const override;
};

struct VariantInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Variant; }
    LocalId dest;
    const Type* enumType;
    size_t variantIndex;
    std::vector<Operand> args;

    VariantInst(LocalId d, const Type* t, size_t vIdx, std::vector<Operand> a)
        : dest(std::move(d)), enumType(t), variantIndex(vIdx), args(std::move(a)) {}
    std::string toString() const override;
};

struct MakeTraitObjectInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::MakeTraitObject; }
    LocalId dest;
    Operand value;
    const Type* concreteType;
    const Type* targetType;
    std::string vtableMangledName;
    std::vector<std::string> vtableMethods;

    MakeTraitObjectInst(LocalId d, Operand v, const Type* c, const Type* t, std::string vtableName, std::vector<std::string> methods) 
        : dest(std::move(d)), value(std::move(v)), concreteType(c), targetType(t), vtableMangledName(std::move(vtableName)), vtableMethods(std::move(methods)) {}
    std::string toString() const override;
};

struct MakeSliceInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::MakeSlice; }
    LocalId dest;
    Operand basePtr;
    Operand length;

    MakeSliceInst(LocalId d, Operand b, Operand l)
        : dest(std::move(d)), basePtr(std::move(b)), length(std::move(l)) {}
    std::string toString() const override;
};

struct BoundsCheckInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::BoundsCheck; }
    Operand index;
    Operand length;

    BoundsCheckInst(Operand idx, Operand len)
        : index(std::move(idx)), length(std::move(len)) {}
    std::string toString() const override;
};

struct CallInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Call; }
    std::optional<LocalId> dest;
    Operand func;
    std::vector<Operand> args;
    const FunctionType* funcType; // Needed for opaque pointer indirect calls

    CallInst(std::optional<LocalId> d, Operand f, std::vector<Operand> a, const FunctionType* fTy = nullptr)
        : dest(std::move(d)), func(std::move(f)), args(std::move(a)), funcType(fTy) {}
    std::string toString() const override;
};

struct VirtualCallInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::VirtualCall; }
    std::optional<LocalId> dest;
    Operand receiver;
    const Type* traitType;
    size_t methodIndex;
    const FunctionType* methodType;
    std::vector<Operand> args;

    VirtualCallInst(std::optional<LocalId> d, Operand r, const Type* t, size_t mIdx, const FunctionType* mTy, std::vector<Operand> a)
        : dest(std::move(d)), receiver(std::move(r)), traitType(t), methodIndex(mIdx), methodType(mTy), args(std::move(a)) {}
    std::string toString() const override;
};

struct AwaitInst : public Instruction {
    Opcode getOpcode() const override { return Opcode::Await; }
    LocalId dest;
    Operand futureVal;
    const Type* innerType;
    
    AwaitInst(LocalId d, Operand f, const Type* t)
        : dest(std::move(d)), futureVal(std::move(f)), innerType(t) {}
    std::string toString() const override;
};

// =============================================================================
// 4. Terminators
// =============================================================================

struct Terminator {
    virtual ~Terminator() = default;
    virtual std::string toString() const = 0;
    virtual Opcode getOpcode() const = 0;
};

struct JumpTerm : public Terminator {
    Opcode getOpcode() const override { return Opcode::Jump; }
    LabelId target;

    explicit JumpTerm(LabelId t) : target(std::move(t)) {}
    std::string toString() const override;
};

struct BranchTerm : public Terminator {
    Opcode getOpcode() const override { return Opcode::Branch; }
    Operand condition;
    LabelId trueTarget;
    LabelId falseTarget;

    BranchTerm(Operand cond, LabelId t, LabelId f)
        : condition(std::move(cond)), trueTarget(std::move(t)), falseTarget(std::move(f)) {}
    std::string toString() const override;
};

struct RetTerm : public Terminator {
    Opcode getOpcode() const override { return Opcode::Ret; }
    std::optional<Operand> value;

    explicit RetTerm(std::optional<Operand> v = std::nullopt) : value(std::move(v)) {}
    std::string toString() const override;
};

struct UnreachableTerm : public Terminator {
    Opcode getOpcode() const override { return Opcode::Unreachable; }
    std::string toString() const override;
};

struct SwitchTerm : public Terminator {
    Opcode getOpcode() const override { return Opcode::Switch; }
    Operand condition;
    std::vector<std::pair<Number, LabelId>> cases;
    LabelId defaultTarget;

    SwitchTerm(Operand cond, std::vector<std::pair<Number, LabelId>> c, LabelId d)
        : condition(std::move(cond)), cases(std::move(c)), defaultTarget(std::move(d)) {}
    std::string toString() const override;
};

// =============================================================================
// 5. Structure (BasicBlocks, Functions, Module)
// =============================================================================

struct BasicBlock {
    LabelId label;
    std::vector<std::unique_ptr<Instruction>> instructions;
    std::unique_ptr<Terminator> terminator;

    explicit BasicBlock(LabelId l) : label(std::move(l)) {}
    std::string toString() const;
};

struct Param {
    const Type* type;
    LocalId id;
    
    std::string toString() const;
};

struct Function {
    GlobalId name;
    std::vector<Param> params;
    const Type* returnType = nullptr;
    std::vector<std::unique_ptr<BasicBlock>> blocks;
    bool isAsync = false;
    bool isGeneric = false;

    Function() = default;
    Function(GlobalId n, const Type* ret) : name(std::move(n)), returnType(ret) {}
    std::string toString() const;
};

struct ExternFunction {
    GlobalId name;
    std::vector<const Type*> paramTypes;
    const Type* returnType;
    bool isVariadic = false;
    std::string toString() const;
};

struct TypeDecl {
    uint32_t id = 0xFFFFFFFF;
    std::string name; // %Struct or %Enum
    bool isEnum = false;
    std::vector<const Type*> fields; // For structs
    std::vector<std::vector<const Type*>> variants; // For enums
    std::string toString() const;
};

enum class GlobalKind {
    Const,
    Static
};

struct GlobalDecl {
    GlobalId id;
    const Type* type;
    GlobalKind kind;
    ConstantValue initializer;
    Visibility visibility = Visibility::Internal;
    std::string toString() const;
    bool isMutable() const { return kind == GlobalKind::Static; }
};

struct Module {
    std::vector<TypeDecl> typeDecls;
    std::vector<GlobalDecl> globalDecls;
    std::vector<ExternFunction> externFunctions;
    std::vector<std::unique_ptr<Function>> functions;

    std::string toString() const;
};

} // namespace mvir
} // namespace fl
