// =============================================================================
// mellis/runtime/memory.h
//
// Mellis Runtime Memory Allocation ABI
//
// Design:
//   - __mellis_alloc returns null on OOM; panic policy delegated to alloc/std.
//   - __mellis_dealloc takes (size, align) for custom/debug/embedded allocators.
//     Hosted default allocators may ignore size and align.
//   - All functions are thread-safe in the hosted profile.
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// Allocate `size` bytes with alignment `align`.
// Returns: non-null pointer on success, null on OOM.
// align must be a power of 2 and >= 1.
// size == 0 may return a unique non-null dangling pointer.
void* __mellis_alloc(size_t size, size_t align);

// Deallocate memory previously allocated by __mellis_alloc or __mellis_realloc.
// ptr must not be null. size and align must match the original allocation.
// Hosted default allocators may ignore size/align.
void  __mellis_dealloc(void* ptr, size_t size, size_t align);

// Reallocate memory. Returns null on OOM (old allocation is preserved).
// ptr: original pointer (or null, in which case behaves like alloc).
// old_size, new_size, align must be consistent with the original allocation.
void* __mellis_realloc(void* ptr, size_t old_size, size_t new_size, size_t align);

// Optional runtime memory primitives (fallback for embedded/freestanding).
// The compiler prefers LLVM intrinsics for these; these are fallback symbols.
void* __mellis_mem_copy(void* dst, const void* src, size_t n);
void* __mellis_mem_move(void* dst, const void* src, size_t n);
void* __mellis_mem_set(void* dst, int val, size_t n);

#ifdef __cplusplus
} // extern "C"
#endif
