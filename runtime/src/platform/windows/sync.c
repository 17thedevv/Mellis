// =============================================================================
// runtime/src/platform/windows/sync.c
//
// Mellis Runtime — Sync ABI (Windows Platform Implementation)
// =============================================================================

#include "mellis/runtime/sync.h"
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>

MellisMutexHandle __mellis_mutex_create(void) {
    CRITICAL_SECTION* cs = (CRITICAL_SECTION*)malloc(sizeof(CRITICAL_SECTION));
    if (!cs) return NULL;
    InitializeCriticalSection(cs);
    return (MellisMutexHandle)cs;
}

void __mellis_mutex_destroy(MellisMutexHandle handle) {
    DeleteCriticalSection((CRITICAL_SECTION*)handle);
    free(handle);
}

void __mellis_mutex_lock(MellisMutexHandle handle) {
    EnterCriticalSection((CRITICAL_SECTION*)handle);
}

int __mellis_mutex_try_lock(MellisMutexHandle handle) {
    return TryEnterCriticalSection((CRITICAL_SECTION*)handle) ? 1 : 0;
}

void __mellis_mutex_unlock(MellisMutexHandle handle) {
    LeaveCriticalSection((CRITICAL_SECTION*)handle);
}

// Condition Variable (Windows 8+)
MellisCondvarHandle __mellis_condvar_create(void) {
    CONDITION_VARIABLE* cv = (CONDITION_VARIABLE*)malloc(sizeof(CONDITION_VARIABLE));
    if (!cv) return NULL;
    InitializeConditionVariable(cv);
    return (MellisCondvarHandle)cv;
}

void __mellis_condvar_destroy(MellisCondvarHandle handle) {
    free(handle);
}

void __mellis_condvar_wait(MellisCondvarHandle cv, MellisMutexHandle mx) {
    SleepConditionVariableCS((CONDITION_VARIABLE*)cv, (CRITICAL_SECTION*)mx, INFINITE);
}

void __mellis_condvar_signal(MellisCondvarHandle handle) {
    WakeConditionVariable((CONDITION_VARIABLE*)handle);
}

void __mellis_condvar_broadcast(MellisCondvarHandle handle) {
    WakeAllConditionVariable((CONDITION_VARIABLE*)handle);
}
