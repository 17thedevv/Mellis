// =============================================================================
// mellis/runtime/panic.h
//
// Mellis Runtime Panic / Trap ABI
//
// Design:
//   - All panic functions are [[noreturn]] / __attribute__((noreturn)).
//   - Default behavior: abort. Unwinding is opt-in and not supported in
//     freestanding/embedded profiles.
//   - __mellis_panic uses PanicInfo* for structured, scalable payloads.
//   - Bounds/div-zero/assert failures are separate entry points for
//     better diagnostics and size-optimization in release builds.
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// Primary panic entry point. Called by compiler-generated code via MVIR PanicInst.
// info must not be null; embedded builds may use a partial PanicInfo.
MELLIS_NORETURN void __mellis_panic(const MellisPanicInfo* info);

// Array/slice index out of bounds.
MELLIS_NORETURN void __mellis_bounds_fail(size_t idx, size_t len,
                                          const char* file, uint32_t line);

// Integer division by zero.
MELLIS_NORETURN void __mellis_div_zero_fail(const char* file, uint32_t line);

// Explicit assertion failure (e.g. assert(condition) in Mellis source).
MELLIS_NORETURN void __mellis_assert_fail(const char* msg,
                                          const char* file, uint32_t line);

// Integer overflow (if language semantics enable checked overflow).
MELLIS_NORETURN void __mellis_overflow_fail(const char* file, uint32_t line);

#ifdef __cplusplus
} // extern "C"
#endif
