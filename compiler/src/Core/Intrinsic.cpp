#include "mellis/Core/Intrinsic.h"

namespace fl {

IntrinsicRegistry::IntrinsicRegistry() {
    intrinsics_ = {
        {"sizeof", {IntrinsicKind::SizeOf, IntrinsicClass::TypeQuery, IntrinsicPhase::CompileTime, true, true, false, false}},
        {"alignof", {IntrinsicKind::AlignOf, IntrinsicClass::TypeQuery, IntrinsicPhase::CompileTime, true, true, false, false}},
        {"typeof", {IntrinsicKind::TypeOf, IntrinsicClass::TypeQuery, IntrinsicPhase::CompileTime, false, true, false, false}},
        {"is_sized", {IntrinsicKind::IsSized, IntrinsicClass::TypeQuery, IntrinsicPhase::CompileTime, true, true, false, false}},
        {"needs_drop", {IntrinsicKind::NeedsDrop, IntrinsicClass::TypeQuery, IntrinsicPhase::CompileTime, true, true, false, false}},
        
        {"ptr_add", {IntrinsicKind::PtrAdd, IntrinsicClass::Pointer, IntrinsicPhase::LowerToMVIR, false, false, false, true}},
        {"ptr_sub", {IntrinsicKind::PtrSub, IntrinsicClass::Pointer, IntrinsicPhase::LowerToMVIR, false, false, false, true}},
        {"ptr_offset", {IntrinsicKind::PtrOffset, IntrinsicClass::Pointer, IntrinsicPhase::LowerToMVIR, false, false, false, true}},
        {"ptr_diff", {IntrinsicKind::PtrDiff, IntrinsicClass::Pointer, IntrinsicPhase::LowerToMVIR, false, false, false, true}},
        {"ptr_read", {IntrinsicKind::PtrRead, IntrinsicClass::Pointer, IntrinsicPhase::LowerToLLVM, false, false, true, true}},
        {"ptr_write", {IntrinsicKind::PtrWrite, IntrinsicClass::Pointer, IntrinsicPhase::LowerToLLVM, false, false, true, true}},
        
        {"str_len", {IntrinsicKind::StrLen, IntrinsicClass::String, IntrinsicPhase::LowerToMVIR, false, false, false, false}},
        {"str_ptr", {IntrinsicKind::StrPtr, IntrinsicClass::String, IntrinsicPhase::LowerToMVIR, false, false, false, false}},
        
        {"mem_copy", {IntrinsicKind::MemCopy, IntrinsicClass::Memory, IntrinsicPhase::LowerToLLVM, true, false, true, true}},
        {"mem_move", {IntrinsicKind::MemMove, IntrinsicClass::Memory, IntrinsicPhase::LowerToLLVM, true, false, true, true}},
        {"mem_set", {IntrinsicKind::MemSet, IntrinsicClass::Memory, IntrinsicPhase::LowerToLLVM, true, false, true, true}},
        
        {"unreachable", {IntrinsicKind::Unreachable, IntrinsicClass::Optimization, IntrinsicPhase::LowerToLLVM, false, false, false, false}},
        {"assume", {IntrinsicKind::Assume, IntrinsicClass::Optimization, IntrinsicPhase::LowerToLLVM, false, false, false, false}}
    };
}

const IntrinsicRegistry& IntrinsicRegistry::get() {
    static IntrinsicRegistry instance;
    return instance;
}

std::optional<IntrinsicDecl> IntrinsicRegistry::lookup(const std::string& name) const {
    auto it = intrinsics_.find(name);
    if (it != intrinsics_.end()) {
        return it->second;
    }
    return std::nullopt;
}

bool IntrinsicRegistry::isIntrinsic(const std::string& name) const {
    return intrinsics_.find(name) != intrinsics_.end();
}

} // namespace fl
