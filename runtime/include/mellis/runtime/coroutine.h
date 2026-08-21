// =============================================================================
// mellis/runtime/coroutine.h
//
// Mellis Runtime Coroutine Frame ABI
//
// Design:
//   Coroutine ABI != Async Language Semantics != Executor API.
//
//   Compiler: handles coroutine lowering and suspend point generation.
//   Runtime:  handles frame allocation / destruction only.
//   Std:      provides the executor (event loop). Runtime does NOT include one.
//
//   The runtime knows nothing about async scheduling or waker semantics.
//   Those belong in std or user-space library code.
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// Allocate a coroutine frame of `size` bytes with alignment `align`.
// Called by compiler-generated coroutine initialization code.
void* __mellis_coroutine_alloc(size_t size, size_t align);

// Deallocate a coroutine frame previously allocated by __mellis_coroutine_alloc.
// Called by compiler-generated coroutine destructor code.
void  __mellis_coroutine_free(void* frame, size_t size, size_t align);

#ifdef __cplusplus
} // extern "C"
#endif
