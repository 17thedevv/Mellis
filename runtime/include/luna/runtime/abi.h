// =============================================================================
// luna/runtime/abi.h
//
// Luna Runtime ABI v1 — Master Header (FROZEN)
// =============================================================================

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LUNA_RUNTIME_ABI_VERSION 1
#define LUNA_MAX_ALIGN 4096

#if defined(__GNUC__) || defined(__clang__)
    #define LUNA_NORETURN __attribute__((noreturn))
#elif defined(_MSC_VER)
    #define LUNA_NORETURN __declspec(noreturn)
#else
    #define LUNA_NORETURN
#endif

// Runtime error codes (stable identifiers)
#define LUNA_ERR_ALLOC_FAILURE     1
#define LUNA_ERR_BOUNDS_VIOLATION  2
#define LUNA_ERR_DIV_ZERO          3
#define LUNA_ERR_ASSERT_FAILURE    4
#define LUNA_ERR_INVALID_STATE     5

// Runtime status codes (stable identifiers)
typedef int32_t LunaStatus;

#define LUNA_STATUS_OK                 ((LunaStatus)0)
#define LUNA_STATUS_EOF                ((LunaStatus)1)
#define LUNA_STATUS_NOT_FOUND          ((LunaStatus)-1)
#define LUNA_STATUS_PERMISSION_DENIED  ((LunaStatus)-2)
#define LUNA_STATUS_ALREADY_EXISTS     ((LunaStatus)-3)
#define LUNA_STATUS_INVALID_ARGUMENT   ((LunaStatus)-4)
#define LUNA_STATUS_IO_ERROR           ((LunaStatus)-5)
#define LUNA_STATUS_OUT_OF_MEMORY      ((LunaStatus)-6)

// Runtime ABI marker function
int __luna_runtime_abi_version(void);

#ifdef __cplusplus
} // extern "C"
#endif
