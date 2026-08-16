// =============================================================================
// runtime/src/platform/windows/threading.c
//
// Mellis Runtime — Thread ABI (Windows Platform Implementation)
// =============================================================================

#include "mellis/runtime/threading.h"
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

typedef struct {
    MellisThreadFn fn;
    void*          arg;
} WinThreadArgs;

static DWORD WINAPI win_thread_trampoline(LPVOID param) {
    WinThreadArgs* args = (WinThreadArgs*)param;
    args->fn(args->arg);
    // NOTE: args struct must be freed by the thread fn or the runtime.
    // For now, we leak it (stdlib wrapper will manage lifetime properly).
    return 0;
}

MellisThreadHandle __mellis_thread_spawn(MellisThreadFn fn, void* arg) {
    WinThreadArgs* args = (WinThreadArgs*)malloc(sizeof(WinThreadArgs));
    if (!args) return NULL;
    args->fn  = fn;
    args->arg = arg;
    HANDLE h = CreateThread(NULL, 0, win_thread_trampoline, args, 0, NULL);
    if (!h) { free(args); return NULL; }
    return (MellisThreadHandle)h;
}

void __mellis_thread_join(MellisThreadHandle handle) {
    WaitForSingleObject((HANDLE)handle, INFINITE);
    CloseHandle((HANDLE)handle);
}

void __mellis_thread_yield(void) {
    SwitchToThread();
}

void __mellis_thread_sleep(uint64_t nanoseconds) {
    DWORD ms = (DWORD)(nanoseconds / 1000000ULL);
    if (ms == 0 && nanoseconds > 0) ms = 1;
    Sleep(ms);
}

uint64_t __mellis_thread_current_id(void) {
    return (uint64_t)GetCurrentThreadId();
}
