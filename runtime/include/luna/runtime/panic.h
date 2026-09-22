// =============================================================================
// luna/runtime/panic.h
//
// Luna Runtime Panic / Trap ABI (FROZEN)
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// --- Tier 1: Public Stable ABI Contract ---------------------------------------

// Primary public panic entrypoint with source location
LUNA_NORETURN void __luna_panic(
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);

// Zero-arg simple panic entrypoint (for stdlib / unwrap aborts)
LUNA_NORETURN void __luna_panic_default(void);

// Array/slice index out of bounds trap
LUNA_NORETURN void __luna_bounds_fail(
    size_t         index,
    size_t         len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);

// --- Tier 2: Internal Runtime Implementation Symbols (Private) ----------------

// Internal panic with explicit error code
LUNA_NORETURN void __luna_panic_code(
    uint32_t       error_code,
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);

// Specific condition traps
LUNA_NORETURN void __luna_div_zero_fail(const char* file, uint32_t line);
LUNA_NORETURN void __luna_assert_fail(const char* msg, const char* file, uint32_t line);
LUNA_NORETURN void __luna_overflow_fail(const char* file, uint32_t line);

#ifdef __cplusplus
} // extern "C"
#endif
