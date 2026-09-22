// =============================================================================
// luna/runtime/threading.h
//
// Luna Runtime Thread ABI — Low-level OS/thread bindings only.
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef void* LunaThreadHandle;
typedef void (*LunaThreadFn)(void* arg);

// Spawn a new thread. Returns handle on success, null on failure.
LunaThreadHandle __luna_thread_spawn(LunaThreadFn fn, void* arg);

// Block until the thread identified by handle exits.
void __luna_thread_join(LunaThreadHandle handle);

// Yield the current thread's timeslice.
void __luna_thread_yield(void);

// Sleep the current thread for at least `nanoseconds` nanoseconds.
void __luna_thread_sleep(uint64_t nanoseconds);

// Returns a platform-unique integer identifier for the current thread.
uint64_t __luna_thread_current_id(void);

#ifdef __cplusplus
} // extern "C"
#endif
