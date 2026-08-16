// =============================================================================
// runtime/src/startup/startup.c
//
// Mellis Runtime — Program Startup / Shutdown (Hosted)
//
// Responsibilities:
//   - Store argc/argv for std::env access.
//   - Run global statics initialization hooks.
//   - After Mellis main returns, run global destructors, then exit.
// =============================================================================

#include "mellis/runtime/process.h"
#include <stdio.h>
#include <stdlib.h>

// Stored by __mellis_startup for access by std::env functions.
static int    g_argc = 0;
static char** g_argv = NULL;

// Global init/fini function table (compiler-generated, registered via pragma/ctor).
typedef void (*MellisInitFn)(void);

// Called from platform entry (CRT main, _start, etc).
void __mellis_startup(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
    // Global statics initialization runs via CRT constructor mechanism
    // (platform-specific __attribute__((constructor)) or .init_array).
    // Nothing explicit needed here in the hosted libc profile.
}

// Called after Mellis main returns.
MELLIS_NORETURN void __mellis_shutdown(int exit_code) {
    // Global destructors run via CRT destructor mechanism.
    exit(exit_code);
}

// Unconditional abort.
MELLIS_NORETURN void __mellis_abort(void) {
    abort();
}

// Accessors for std::env (internal use).
int    __mellis_argc(void)  { return g_argc; }
char** __mellis_argv(void)  { return g_argv; }
