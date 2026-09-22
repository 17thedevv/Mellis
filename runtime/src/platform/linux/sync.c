// =============================================================================
// runtime/src/platform/linux/sync.c
//
// Luna Runtime — Sync ABI (Linux/POSIX Platform Implementation)
// =============================================================================

#include "luna/runtime/sync.h"
#include <pthread.h>
#include <stdlib.h>

LunaMutexHandle __luna_mutex_create(void) {
    pthread_mutex_t* mutex = (pthread_mutex_t*)malloc(sizeof(pthread_mutex_t));
    if (!mutex) return NULL;
    if (pthread_mutex_init(mutex, NULL) != 0) {
        free(mutex);
        return NULL;
    }
    return (LunaMutexHandle)mutex;
}

void __luna_mutex_destroy(LunaMutexHandle handle) {
    if (!handle) return;
    pthread_mutex_destroy((pthread_mutex_t*)handle);
    free(handle);
}

void __luna_mutex_lock(LunaMutexHandle handle) {
    if (handle) {
        pthread_mutex_lock((pthread_mutex_t*)handle);
    }
}

int __luna_mutex_try_lock(LunaMutexHandle handle) {
    if (!handle) return 0;
    return pthread_mutex_trylock((pthread_mutex_t*)handle) == 0 ? 1 : 0;
}

void __luna_mutex_unlock(LunaMutexHandle handle) {
    if (handle) {
        pthread_mutex_unlock((pthread_mutex_t*)handle);
    }
}

// Condition Variable (POSIX pthreads)
LunaCondvarHandle __luna_condvar_create(void) {
    pthread_cond_t* cv = (pthread_cond_t*)malloc(sizeof(pthread_cond_t));
    if (!cv) return NULL;
    if (pthread_cond_init(cv, NULL) != 0) {
        free(cv);
        return NULL;
    }
    return (LunaCondvarHandle)cv;
}

void __luna_condvar_destroy(LunaCondvarHandle handle) {
    if (!handle) return;
    pthread_cond_destroy((pthread_cond_t*)handle);
    free(handle);
}

void __luna_condvar_wait(LunaCondvarHandle cv, LunaMutexHandle mx) {
    if (cv && mx) {
        pthread_cond_wait((pthread_cond_t*)cv, (pthread_mutex_t*)mx);
    }
}

void __luna_condvar_signal(LunaCondvarHandle handle) {
    if (handle) {
        pthread_cond_signal((pthread_cond_t*)handle);
    }
}

void __luna_condvar_broadcast(LunaCondvarHandle handle) {
    if (handle) {
        pthread_cond_broadcast((pthread_cond_t*)handle);
    }
}
