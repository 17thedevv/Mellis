// =============================================================================
// runtime/src/platform/linux/threading.c
//
// Mellis Runtime — Thread ABI (Linux/POSIX Platform Implementation)
// =============================================================================

#include "mellis/runtime/threading.h"
#include <pthread.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

typedef struct {
    MellisThreadFn fn;
    void*          arg;
} PosixThreadArgs;

static void* posix_thread_trampoline(void* param) {
    PosixThreadArgs* args = (PosixThreadArgs*)param;
    args->fn(args->arg);
    free(args);
    return NULL;
}

MellisThreadHandle __mellis_thread_spawn(MellisThreadFn fn, void* arg) {
    PosixThreadArgs* args = (PosixThreadArgs*)malloc(sizeof(PosixThreadArgs));
    if (!args) return NULL;
    args->fn  = fn;
    args->arg = arg;
    pthread_t* t = (pthread_t*)malloc(sizeof(pthread_t));
    if (!t) { free(args); return NULL; }
    if (pthread_create(t, NULL, posix_thread_trampoline, args) != 0) {
        free(args); free(t); return NULL;
    }
    return (MellisThreadHandle)t;
}

void __mellis_thread_join(MellisThreadHandle handle) {
    pthread_join(*(pthread_t*)handle, NULL);
    free(handle);
}

void __mellis_thread_yield(void) {
    sched_yield();
}

void __mellis_thread_sleep(uint64_t nanoseconds) {
    struct timespec ts;
    ts.tv_sec  = (time_t)(nanoseconds / 1000000000ULL);
    ts.tv_nsec = (long)(nanoseconds   % 1000000000ULL);
    nanosleep(&ts, NULL);
}

uint64_t __mellis_thread_current_id(void) {
    return (uint64_t)pthread_self();
}
