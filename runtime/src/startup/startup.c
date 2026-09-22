// =============================================================================
// runtime/src/startup/startup.c
//
// Luna Runtime — Program Startup / Shutdown (Hosted)
// =============================================================================

#include "luna/runtime/process.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Stored by __luna_startup for access by std::process functions.
static int    g_argc = 0;
static char** g_argv = NULL;

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

// Disable MinGW CRT command-line wildcard expansion
int _CRT_glob = 0;

typedef LPWSTR* (WINAPI *pfn_CommandLineToArgvW)(LPCWSTR lpCmdLine, int* pNumArgs);

static int    g_utf8_argc = 0;
static char** g_utf8_argv = NULL;

static void free_utf8_args_win32(void) {
    if (g_utf8_argv) {
        for (int i = 0; i < g_utf8_argc; i++) {
            if (g_utf8_argv[i]) {
                free(g_utf8_argv[i]);
            }
        }
        free(g_utf8_argv);
        g_utf8_argv = NULL;
        g_utf8_argc = 0;
    }
}

static void init_utf8_args_win32(int argc, char** argv) {
    free_utf8_args_win32();

    HMODULE hShell = LoadLibraryA("shell32.dll");
    if (!hShell) return;
    pfn_CommandLineToArgvW pCommandLineToArgvW = (pfn_CommandLineToArgvW)GetProcAddress(hShell, "CommandLineToArgvW");
    if (!pCommandLineToArgvW) {
        FreeLibrary(hShell);
        return;
    }

    int num_args = 0;
    LPWSTR* w_argv = pCommandLineToArgvW(GetCommandLineW(), &num_args);
    if (w_argv) {
        // If simulated arguments were provided by a test (where argv[0] does not match host executable),
        // don't override with host process command line.
        if (argv != NULL && argc > 0 && argv[0] != NULL) {
            int utf0_len = WideCharToMultiByte(CP_UTF8, 0, w_argv[0], -1, NULL, 0, NULL, NULL);
            char* exe_utf8 = (char*)malloc(utf0_len > 0 ? utf0_len : 1);
            if (utf0_len > 0) {
                WideCharToMultiByte(CP_UTF8, 0, w_argv[0], -1, exe_utf8, utf0_len, NULL, NULL);
            } else {
                exe_utf8[0] = '\0';
            }

            int is_real = 0;
            if (strstr(exe_utf8, argv[0]) != NULL || strstr(argv[0], exe_utf8) != NULL) {
                is_real = 1;
            }
            free(exe_utf8);

            if (is_real) {
                g_utf8_argc = num_args;
                g_utf8_argv = (char**)calloc(num_args + 1, sizeof(char*));
                for (int i = 0; i < num_args; i++) {
                    int utf8_len = WideCharToMultiByte(CP_UTF8, 0, w_argv[i], -1, NULL, 0, NULL, NULL);
                    if (utf8_len > 0) {
                        g_utf8_argv[i] = (char*)malloc(utf8_len);
                        WideCharToMultiByte(CP_UTF8, 0, w_argv[i], -1, g_utf8_argv[i], utf8_len, NULL, NULL);
                    } else {
                        g_utf8_argv[i] = (char*)calloc(1, 1);
                    }
                }
            }
        }
        LocalFree(w_argv);
    }
    FreeLibrary(hShell);
}
#endif

// Called from compiler-generated entrypoint @main.
void __luna_startup(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
#ifdef _WIN32
    init_utf8_args_win32(argc, argv);
#endif
    // Set stdout and stderr to unbuffered or line-buffered
    setvbuf(stdout, NULL, _IONBF, 0);
}

// Called after Luna main returns.
LUNA_NORETURN void __luna_shutdown(int exit_code) {
#ifdef _WIN32
    free_utf8_args_win32();
#endif
    fflush(stdout);
    fflush(stderr);
    exit(exit_code);
}

// Unconditional abort.
LUNA_NORETURN void __luna_abort(void) {
    fflush(stdout);
    fflush(stderr);
    abort();
}

// Structured Process Invocation Arguments (ABI v1)

size_t __luna_process_arg_count(void) {
#ifdef _WIN32
    if (g_utf8_argv && g_utf8_argc > 1) {
        return (size_t)(g_utf8_argc - 1);
    }
#endif
    if (g_argc <= 1 || g_argv == NULL) {
        return 0;
    }
    return (size_t)(g_argc - 1);
}

int32_t __luna_process_arg(size_t index, const uint8_t** out_ptr, size_t* out_len) {
    if (!out_ptr || !out_len) {
        return LUNA_STATUS_INVALID_ARGUMENT;
    }
#ifdef _WIN32
    if (g_utf8_argv && g_utf8_argc > 1) {
        size_t count = (size_t)(g_utf8_argc - 1);
        if (index >= count) {
            *out_ptr = NULL;
            *out_len = 0;
            return LUNA_STATUS_INVALID_ARGUMENT;
        }
        const char* arg = g_utf8_argv[index + 1];
        if (!arg) {
            *out_ptr = NULL;
            *out_len = 0;
            return LUNA_STATUS_INVALID_ARGUMENT;
        }
        *out_ptr = (const uint8_t*)arg;
        *out_len = strlen(arg);
        return LUNA_STATUS_OK;
    }
#endif
    if (g_argc <= 1 || g_argv == NULL) {
        *out_ptr = NULL;
        *out_len = 0;
        return LUNA_STATUS_INVALID_ARGUMENT;
    }
    size_t count = (size_t)(g_argc - 1);
    if (index >= count) {
        *out_ptr = NULL;
        *out_len = 0;
        return LUNA_STATUS_INVALID_ARGUMENT;
    }
    const char* arg = g_argv[index + 1];
    if (!arg) {
        *out_ptr = NULL;
        *out_len = 0;
        return LUNA_STATUS_INVALID_ARGUMENT;
    }
    *out_ptr = (const uint8_t*)arg;
    *out_len = strlen(arg);
    return LUNA_STATUS_OK;
}

int32_t __luna_process_executable(const uint8_t** out_ptr, size_t* out_len) {
    if (!out_ptr || !out_len) {
        return LUNA_STATUS_INVALID_ARGUMENT;
    }
#ifdef _WIN32
    if (g_utf8_argv && g_utf8_argc >= 1 && g_utf8_argv[0] != NULL) {
        *out_ptr = (const uint8_t*)g_utf8_argv[0];
        *out_len = strlen(g_utf8_argv[0]);
        return LUNA_STATUS_OK;
    }
#endif
    if (g_argc < 1 || g_argv == NULL || g_argv[0] == NULL) {
        *out_ptr = NULL;
        *out_len = 0;
        return LUNA_STATUS_NOT_FOUND;
    }
    *out_ptr = (const uint8_t*)g_argv[0];
    *out_len = strlen(g_argv[0]);
    return LUNA_STATUS_OK;
}

// Low-level CRT accessors (internal runtime / test use only)
int    __luna_argc(void)  { return g_argc; }
char** __luna_argv(void)  { return g_argv; }
