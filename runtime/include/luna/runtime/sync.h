// =============================================================================
// luna/runtime/sync.h
//
// Luna Runtime Synchronization ABI — Native mutex/condvar OS bindings.
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef void* LunaMutexHandle;
typedef void* LunaCondvarHandle;

// Mutex
LunaMutexHandle __luna_mutex_create(void);
void            __luna_mutex_destroy(LunaMutexHandle handle);
void            __luna_mutex_lock(LunaMutexHandle handle);
int             __luna_mutex_try_lock(LunaMutexHandle handle); // 1=acquired
void            __luna_mutex_unlock(LunaMutexHandle handle);

// Condition Variable
LunaCondvarHandle __luna_condvar_create(void);
void              __luna_condvar_destroy(LunaCondvarHandle handle);
void              __luna_condvar_wait(LunaCondvarHandle cv,
                                      LunaMutexHandle mx);
void              __luna_condvar_signal(LunaCondvarHandle handle);
void              __luna_condvar_broadcast(LunaCondvarHandle handle);

#ifdef __cplusplus
} // extern "C"
#endif
