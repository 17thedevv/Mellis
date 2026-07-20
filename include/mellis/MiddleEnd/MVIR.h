// =============================================================================
// mellis/MiddleEnd/MVIR.h
//
// FreedomLanguage Intermediate Representation (MVIR).
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
    std::string name; // e.g., "%0", "%x"
    std::string toString() const { return name; }
    bool operator==(const LocalId& other) const { return name == other.name; }
};

struct GlobalId {
    std::string name; // e.g., "@main"
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

using Operand = std::variant<LocalId, GlobalId, Number, Boolean>;

std::string toString(const Operand& op);

// =============================================================================
// 3. Instructions
// =============================================================================

struct Instruction {
    virtual ~Instruction() = default;
    virtual std::string toString() const = 0;
};

// ── Memory Instructions ──────────────────────────────────────────────────────

struct LocalInst : public Instruction {
    LocalId dest;
    const Type* type;

    LocalInst(LocalId d, const Type* t) : dest(std::move(d)), type(t) {}
    std::string toString() const override;
};

struct LoadInst : public Instruction {
    LocalId dest;
    const Type* type;
    Operand ptr;

    LoadInst(LocalId d, const Type* t, Operand p) : dest(std::move(d)), type(t), ptr(std::move(p)) {}
    std::string toString() const override;
};

struct StoreInst : public Instruction {
    const Type* type;
    Operand value;
    Operand ptr;

    StoreInst(const Type* t, Operand v, Operand p) : type(t), value(std::move(v)), ptr(std::move(p)) {}
    std::string toString() const override;
};

struct IndexInst : public Instruction {
    LocalId dest;
    const Type* type;
    Operand base;
    Operand index;

    IndexInst(LocalId d, const Type* t, Operand b, Operand idx) : dest(std::move(d)), type(t), base(std::move(b)), index(std::move(idx)) {}
    std::string toString() const override;
};

struct FieldInst : public Instruction {
    LocalId dest;
    const Type* type;
    Operand base;
    size_t index;

    FieldInst(LocalId d, const Type* t, Operand b, size_t idx) : dest(std::move(d)), type(t), base(std::move(b)), index(idx) {}
    std::string toString() const override;
};



struct BorrowInst : public Instruction {
    LocalId dest;
    bool isMutable;
    Operand base;

    BorrowInst(LocalId d, bool m, Operand b) : dest(std::move(d)), isMutable(m), base(std::move(b)) {}
    std::string toString() const override;
};

struct CastInst : public Instruction {
    LocalId dest;
    Operand value;
    const Type* targetType;

    CastInst(LocalId d, Operand v, const Type* t) : dest(std::move(d)), value(std::move(v)), targetType(t) {}
    std::string toString() const override;
};

struct DropInst : public Instruction {
    Operand value;
    const Type* type;

    DropInst(Operand v, const Type* t) : value(std::move(v)), type(t) {}
    std::string toString() const override;
};

struct SizeofInst : public Instruction {
    LocalId dest;
    const Type* targetType;

    SizeofInst(LocalId d, const Type* t) : dest(std::move(d)), targetType(t) {}
    std::string toString() const override;
};

struct AlignofInst : public Instruction {
    LocalId dest;
    const Type* targetType;

    AlignofInst(LocalId d, const Type* t) : dest(std::move(d)), targetType(t) {}
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
    LocalId dest;
    AluOp op;
    Operand left;
    Operand right;

    AluInst(LocalId d, AluOp o, Operand l, Operand r)
        : dest(std::move(d)), op(o), left(std::move(l)), right(std::move(r)) {}
    std::string toString() const override;
};

struct UnaryInst : public Instruction {
    LocalId dest;
    UnaryOp op;
    Operand operand;

    UnaryInst(LocalId d, UnaryOp o, Operand v)
        : dest(std::move(d)), op(o), operand(std::move(v)) {}
    std::string toString() const override;
};

// ── Call Instruction ─────────────────────────────────────────────────────────

struct ExtractInst : public Instruction {
    LocalId dest;
    Operand base;
    std::vector<const Type*> payloadTypes;
    size_t variantIndex;
    size_t fieldIndex;

    ExtractInst(LocalId d, Operand b, std::vector<const Type*> pTypes, size_t vIdx, size_t fIdx)
        : dest(std::move(d)), base(std::move(b)), payloadTypes(std::move(pTypes)), variantIndex(vIdx), fieldIndex(fIdx) {}
    std::string toString() const override;
};

struct TagInst : public Instruction {
    LocalId dest;
    Operand base;

    TagInst(LocalId d, Operand b) : dest(std::move(d)), base(std::move(b)) {}
    std::string toString() const override;
};

struct VariantInst : public Instruction {
    LocalId dest;
    const Type* enumType;
    size_t variantIndex;
    std::vector<Operand> args;

    VariantInst(LocalId d, const Type* t, size_t vIdx, std::vector<Operand> a)
        : dest(std::move(d)), enumType(t), variantIndex(vIdx), args(std::move(a)) {}
    std::string toString() const override;
};

struct MakeTraitObjectInst : public Instruction {
    LocalId dest;
    Operand value;
    const Type* concreteType;
    const Type* targetType;
    std::vector<SymbolID> vtableMethods;

    MakeTraitObjectInst(LocalId d, Operand v, const Type* c, const Type* t, std::vector<SymbolID> methods) 
        : dest(std::move(d)), value(std::move(v)), concreteType(c), targetType(t), vtableMethods(std::move(methods)) {}
    std::string toString() const override;
};

struct CallInst : public Instruction {
    std::optional<LocalId> dest;
    Operand func;
    std::vector<Operand> args;
    const FunctionType* funcType; // Needed for opaque pointer indirect calls

    CallInst(std::optional<LocalId> d, Operand f, std::vector<Operand> a, const FunctionType* fTy = nullptr)
        : dest(std::move(d)), func(std::move(f)), args(std::move(a)), funcType(fTy) {}
    std::string toString() const override;
};

struct VirtualCallInst : public Instruction {
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
};

struct JumpTerm : public Terminator {
    LabelId target;

    explicit JumpTerm(LabelId t) : target(std::move(t)) {}
    std::string toString() const override;
};

struct BranchTerm : public Terminator {
    Operand condition;
    LabelId trueTarget;
    LabelId falseTarget;

    BranchTerm(Operand cond, LabelId t, LabelId f)
        : condition(std::move(cond)), trueTarget(std::move(t)), falseTarget(std::move(f)) {}
    std::string toString() const override;
};

struct RetTerm : public Terminator {
    std::optional<Operand> value;

    explicit RetTerm(std::optional<Operand> v = std::nullopt) : value(std::move(v)) {}
    std::string toString() const override;
};

struct SwitchTerm : public Terminator {
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
    const Type* returnType;
    std::vector<std::unique_ptr<BasicBlock>> blocks;
    bool isAsync = false;

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
    std::string name; // %Struct
    std::vector<const Type*> fields;
    std::string toString() const;
};

struct GlobalDecl {
    GlobalId name;
    const Type* type;
    std::string stringLiteral;
    std::string toString() const;
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
