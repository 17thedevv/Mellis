// =============================================================================
// mellis/runtime/process.h
//
// Mellis Runtime Process Startup / Shutdown ABI
//
// Design:
//   - Platform Entry (CRT / _start / WinMain) calls __mellis_startup.
//   - __mellis_startup initializes runtime context, runs global statics init,
//     then calls the user-defined main entry.
//   - __mellis_shutdown performs reverse teardown.
//   - ABI differs slightly per profile (hosted vs freestanding vs embedded).
//   - In freestanding, __mellis_startup may receive no argc/argv.
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// Called from the platform entry point (CRT, custom _start, etc).
// In hosted: argc and argv are provided by the OS.
// In freestanding/embedded: argc=0, argv=null.
void __mellis_startup(int argc, char** argv);

// Called after Mellis main() returns. Performs global destructors, flushes
// runtime resources, then exits the process with the given exit code.
MELLIS_NORETURN void __mellis_shutdown(int exit_code);

// Immediate unconditional process abort.
MELLIS_NORETURN void __mellis_abort(void);

#ifdef __cplusplus
} // extern "C"
#endif
