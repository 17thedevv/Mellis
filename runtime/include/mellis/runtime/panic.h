// =============================================================================
// mellis/runtime/panic.h
//
// Mellis Runtime Panic / Trap ABI (FROZEN)
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// --- Tier 1: Public Stable ABI Contract ---------------------------------------

// Primary public panic entrypoint
MELLIS_NORETURN void __mellis_panic(
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);

// Array/slice index out of bounds trap
MELLIS_NORETURN void __mellis_bounds_fail(
    size_t         index,
    size_t         len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);

// --- Tier 2: Internal Runtime Implementation Symbols (Private) ----------------

// Internal panic with explicit error code
MELLIS_NORETURN void __mellis_panic_code(
    uint32_t       error_code,
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);

// Backward-compatible traps
MELLIS_NORETURN void __mellis_div_zero_fail(const char* file, uint32_t line);
MELLIS_NORETURN void __mellis_assert_fail(const char* msg, const char* file, uint32_t line);
MELLIS_NORETURN void __mellis_overflow_fail(const char* file, uint32_t line);

#ifdef __cplusplus
} // extern "C"
#endif
