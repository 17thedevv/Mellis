#pragma once

#include <cstdint>
#include <string>
#include <optional>
#include <unordered_map>

namespace fl {

enum class IntrinsicKind : uint32_t {
    None = 0,
    
    // Type & Compile-Time Introspection
    SizeOf,
    AlignOf,
    TypeOf,
    IsSized,
    NeedsDrop,
    
    // Memory & Pointer Operations
    PtrOffset,
    PtrAdd,
    PtrSub,
    PtrDiff,
    PtrRead,
    PtrWrite,
    MemCopy,
    MemMove,
    MemSet,
    
    // String Primitives
    StrLen,
    StrPtr,
    
    // Optimization Hints
    Unreachable,
    Assume,
    Likely,
    Unlikely,
    
    // Atomics
    AtomicLoad,
    AtomicStore,
    AtomicExchange,
    AtomicCompareExchange,
    AtomicFence
};

enum class IntrinsicClass : uint8_t {
    TypeQuery,
    Memory,
    Pointer,
    String,
    Optimization,
    Atomic,
    Runtime
};

enum class IntrinsicPhase : uint8_t {
    CompileTime,      // Folded completely by TypeChecker/ComptimeEval
    LowerToMVIR,      // Emits regular MVIR instructions
    LowerToLLVM,      // Passed as IntrinsicCallInst down to LLVM
    RuntimeABI        // Resolved to an external runtime library call
};

struct IntrinsicDecl {
    IntrinsicKind kind;
    IntrinsicClass category;
    IntrinsicPhase phase;
    
    bool acceptsTypeArgs;
    bool constEvaluable;
    bool mayEmitRuntimeCall;
    bool requiresUnsafe;
};

class IntrinsicRegistry {
private:
    std::unordered_map<std::string, IntrinsicDecl> intrinsics_;

    IntrinsicRegistry(); // Private constructor for singleton

public:
    static const IntrinsicRegistry& get();

    std::optional<IntrinsicDecl> lookup(const std::string& name) const;
    bool isIntrinsic(const std::string& name) const;
};

} // namespace fl
