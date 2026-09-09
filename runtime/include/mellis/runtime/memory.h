// =============================================================================
// mellis/runtime/memory.h
//
// Mellis Runtime Memory Allocation ABI (FROZEN)
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// --- Tier 1: Public Stable ABI Contract ---------------------------------------

// Allocate `size` bytes with alignment `align`.
// Returns non-null pointer. On OOM or invalid alignment, panics deterministically.
void* __mellis_alloc(size_t size, size_t align);

// Deallocate memory previously allocated by __mellis_alloc or __mellis_realloc.
// If ptr is null or zero-size sentinel, behaves as no-op.
void  __mellis_dealloc(void* ptr, size_t size, size_t align);

// Reallocate memory block preserving alignment.
void* __mellis_realloc(void* ptr, size_t old_size, size_t old_align, size_t new_size);

// --- Tier 2: Internal Runtime Implementation Symbols (Private) ----------------

// Backward-compatibility alias for __mellis_dealloc.
void  __mellis_free(void* ptr, size_t size, size_t align);

// Query if a pointer is the shared zero-size sentinel (internal tests only).
int   __mellis_is_zero_sentinel(const void* ptr);

// Optional runtime memory primitives (fallback for embedded/freestanding).
void* __mellis_mem_copy(void* dst, const void* src, size_t n);
void* __mellis_mem_move(void* dst, const void* src, size_t n);
void* __mellis_mem_set(void* dst, int val, size_t n);

#ifdef __cplusplus
} // extern "C"
#endif
