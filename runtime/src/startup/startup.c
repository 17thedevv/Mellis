// =============================================================================
// runtime/src/startup/startup.c
//
// Mellis Runtime — Program Startup / Shutdown (Hosted)
// =============================================================================

#include "mellis/runtime/process.h"
#include <stdio.h>
#include <stdlib.h>

// Stored by __mellis_startup for access by std::env functions.
static int    g_argc = 0;
static char** g_argv = NULL;

// Called from compiler-generated entrypoint @main.
void __mellis_startup(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
    // Set stdout and stderr to unbuffered or line-buffered
    setvbuf(stdout, NULL, _IONBF, 0);
}

// Called after Mellis main returns.
MELLIS_NORETURN void __mellis_shutdown(int exit_code) {
    fflush(stdout);
    fflush(stderr);
    exit(exit_code);
}

// Unconditional abort.
MELLIS_NORETURN void __mellis_abort(void) {
    fflush(stdout);
    fflush(stderr);
    abort();
}

// Accessors for std::env (internal use).
int    __mellis_argc(void)  { return g_argc; }
char** __mellis_argv(void)  { return g_argv; }
