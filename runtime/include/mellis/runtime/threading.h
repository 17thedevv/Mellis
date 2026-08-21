// =============================================================================
// mellis/runtime/threading.h
//
// Mellis Runtime Thread ABI — Low-level OS/thread bindings only.
//
// Design:
//   - These are backend primitives for std::thread, NOT public API.
//   - The public Thread type and its API live entirely in std.
//   - Platform implementations in runtime/src/platform/{windows,linux,...}/
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef void* MellisThreadHandle;
typedef void (*MellisThreadFn)(void* arg);

// Spawn a new thread. Returns handle on success, null on failure.
MellisThreadHandle __mellis_thread_spawn(MellisThreadFn fn, void* arg);

// Block until the thread identified by handle exits.
void __mellis_thread_join(MellisThreadHandle handle);

// Yield the current thread's timeslice.
void __mellis_thread_yield(void);

// Sleep the current thread for at least `nanoseconds` nanoseconds.
void __mellis_thread_sleep(uint64_t nanoseconds);

// Returns a platform-unique integer identifier for the current thread.
uint64_t __mellis_thread_current_id(void);

#ifdef __cplusplus
} // extern "C"
#endif
