// =============================================================================
// mellis/runtime/abi.h
//
// Mellis Runtime ABI v1 — Master Header (FROZEN)
// =============================================================================

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define MELLIS_RUNTIME_ABI_VERSION 1
#define MELLIS_MAX_ALIGN 4096

#if defined(__GNUC__) || defined(__clang__)
    #define MELLIS_NORETURN __attribute__((noreturn))
#elif defined(_MSC_VER)
    #define MELLIS_NORETURN __declspec(noreturn)
#else
    #define MELLIS_NORETURN
#endif

// Runtime error codes (stable identifiers)
#define MELLIS_ERR_ALLOC_FAILURE     1
#define MELLIS_ERR_BOUNDS_VIOLATION  2
#define MELLIS_ERR_DIV_ZERO          3
#define MELLIS_ERR_ASSERT_FAILURE    4
#define MELLIS_ERR_INVALID_STATE     5

#ifdef __cplusplus
} // extern "C"
#endif
