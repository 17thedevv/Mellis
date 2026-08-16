// =============================================================================
// mellis/runtime/sync.h
//
// Mellis Runtime Synchronization ABI — Native mutex/condvar OS bindings.
//
// Design:
//   - Runtime provides native OS mutex handle ABI only.
//   - Public Mutex and Condvar data structures live in std.
//   - Atomics are handled by LLVM intrinsics directly; the runtime only
//     provides fallback lock-based implementations for exotic platforms.
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef void* MellisMutexHandle;
typedef void* MellisCondvarHandle;

// Mutex
MellisMutexHandle __mellis_mutex_create(void);
void              __mellis_mutex_destroy(MellisMutexHandle handle);
void              __mellis_mutex_lock(MellisMutexHandle handle);
int               __mellis_mutex_try_lock(MellisMutexHandle handle); // 1=acquired
void              __mellis_mutex_unlock(MellisMutexHandle handle);

// Condition Variable
MellisCondvarHandle __mellis_condvar_create(void);
void                __mellis_condvar_destroy(MellisCondvarHandle handle);
void                __mellis_condvar_wait(MellisCondvarHandle cv,
                                         MellisMutexHandle mx);
void                __mellis_condvar_signal(MellisCondvarHandle handle);
void                __mellis_condvar_broadcast(MellisCondvarHandle handle);

#ifdef __cplusplus
} // extern "C"
#endif
