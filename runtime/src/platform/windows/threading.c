// =============================================================================
// runtime/src/platform/windows/threading.c
//
// Luna Runtime — Thread ABI (Windows Platform Implementation)
// =============================================================================

#include "luna/runtime/threading.h"
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>

typedef struct {
    LunaThreadFn fn;
    void*        arg;
} WinThreadArgs;

static DWORD WINAPI win_thread_trampoline(LPVOID param) {
    WinThreadArgs* args = (WinThreadArgs*)param;
    args->fn(args->arg);
    return 0;
}

LunaThreadHandle __luna_thread_spawn(LunaThreadFn fn, void* arg) {
    WinThreadArgs* args = (WinThreadArgs*)malloc(sizeof(WinThreadArgs));
    if (!args) return NULL;
    args->fn  = fn;
    args->arg = arg;
    HANDLE h = CreateThread(NULL, 0, win_thread_trampoline, args, 0, NULL);
    if (!h) { free(args); return NULL; }
    return (LunaThreadHandle)h;
}

void __luna_thread_join(LunaThreadHandle handle) {
    WaitForSingleObject((HANDLE)handle, INFINITE);
    CloseHandle((HANDLE)handle);
}

void __luna_thread_yield(void) {
    SwitchToThread();
}

void __luna_thread_sleep(uint64_t nanoseconds) {
    DWORD ms = (DWORD)(nanoseconds / 1000000ULL);
    if (ms == 0 && nanoseconds > 0) ms = 1;
    Sleep(ms);
}

uint64_t __luna_thread_current_id(void) {
    return (uint64_t)GetCurrentThreadId();
}
