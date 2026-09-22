// =============================================================================
// runtime/src/platform/windows/sync.c
//
// Luna Runtime — Sync ABI (Windows Platform Implementation)
// =============================================================================

#include "luna/runtime/sync.h"
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>

LunaMutexHandle __luna_mutex_create(void) {
    CRITICAL_SECTION* cs = (CRITICAL_SECTION*)malloc(sizeof(CRITICAL_SECTION));
    if (!cs) return NULL;
    InitializeCriticalSection(cs);
    return (LunaMutexHandle)cs;
}

void __luna_mutex_destroy(LunaMutexHandle handle) {
    DeleteCriticalSection((CRITICAL_SECTION*)handle);
    free(handle);
}

void __luna_mutex_lock(LunaMutexHandle handle) {
    EnterCriticalSection((CRITICAL_SECTION*)handle);
}

int __luna_mutex_try_lock(LunaMutexHandle handle) {
    return TryEnterCriticalSection((CRITICAL_SECTION*)handle) ? 1 : 0;
}

void __luna_mutex_unlock(LunaMutexHandle handle) {
    LeaveCriticalSection((CRITICAL_SECTION*)handle);
}

// Condition Variable (Windows 8+)
LunaCondvarHandle __luna_condvar_create(void) {
    CONDITION_VARIABLE* cv = (CONDITION_VARIABLE*)malloc(sizeof(CONDITION_VARIABLE));
    if (!cv) return NULL;
    InitializeConditionVariable(cv);
    return (LunaCondvarHandle)cv;
}

void __luna_condvar_destroy(LunaCondvarHandle handle) {
    free(handle);
}

void __luna_condvar_wait(LunaCondvarHandle cv, LunaMutexHandle mx) {
    SleepConditionVariableCS((CONDITION_VARIABLE*)cv, (CRITICAL_SECTION*)mx, INFINITE);
}

void __luna_condvar_signal(LunaCondvarHandle handle) {
    WakeConditionVariable((CONDITION_VARIABLE*)handle);
}

void __luna_condvar_broadcast(LunaCondvarHandle handle) {
    WakeAllConditionVariable((CONDITION_VARIABLE*)handle);
}
