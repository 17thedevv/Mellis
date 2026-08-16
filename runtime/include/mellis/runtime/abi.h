// =============================================================================
// mellis/runtime/abi.h
//
// Mellis Runtime ABI v1 — Master Header
//
// This file declares the complete stable ABI exported by the Mellis runtime.
// All symbols are prefixed with __mellis_ and use C-compatible calling
// conventions for portability across compilers and platforms.
//
// ABI Version: 1
// Status: FROZEN
// =============================================================================

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// =============================================================================
// ABI Version
// =============================================================================
#define MELLIS_RUNTIME_ABI_VERSION 1

#if defined(__GNUC__) || defined(__clang__)
    #define MELLIS_NORETURN __attribute__((noreturn))
#elif defined(_MSC_VER)
    #define MELLIS_NORETURN __declspec(noreturn)
#else
    #define MELLIS_NORETURN
#endif

// =============================================================================
// PanicInfo — structured payload passed to __mellis_panic
//
// Embedded targets may receive this with null message if memory is constrained.
// =============================================================================
typedef struct MellisPanicInfo {
    const char* message;     // UTF-8 panic message (may be null in freestanding)
    size_t      message_len;
    const char* file;        // Source file path (may be null)
    uint32_t    line;
    uint32_t    column;
    uint32_t    error_code;  // Stable runtime error code (see below)
} MellisPanicInfo;

// Runtime error codes (stable identifiers)
#define MELLIS_ERR_ALLOC_FAILURE     1
#define MELLIS_ERR_BOUNDS_VIOLATION  2
#define MELLIS_ERR_DIV_ZERO          3
#define MELLIS_ERR_ASSERT_FAILURE    4
#define MELLIS_ERR_INVALID_STATE     5

#ifdef __cplusplus
} // extern "C"
#endif
