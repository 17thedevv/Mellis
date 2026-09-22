// =============================================================================
// luna/runtime/process.h
//
// Luna Runtime Process Startup / Shutdown ABI
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

// Called from the platform entry point (CRT, custom _start, etc).
void __luna_startup(int argc, char** argv);

// Called after Luna main() returns. Performs global destructors, flushes
// runtime resources, then exits the process with the given exit code.
LUNA_NORETURN void __luna_shutdown(int exit_code);

// Immediate unconditional process abort.
LUNA_NORETURN void __luna_abort(void);

// Structured process invocation arguments (ABI v1)
//
// __luna_process_arg_count returns the count of user arguments only (excluding argv[0]).
size_t  __luna_process_arg_count(void);

// __luna_process_arg returns a borrowed byte slice (ptr, len) for user argument at 0-based index.
// Returns LUNA_STATUS_OK on success, or LUNA_STATUS_INVALID_ARGUMENT on out-of-bounds / null out params.
int32_t __luna_process_arg(size_t index, const uint8_t** out_ptr, size_t* out_len);

// __luna_process_executable returns a borrowed byte slice (ptr, len) for argv[0] (invoked executable path).
// Returns LUNA_STATUS_OK on success, or LUNA_STATUS_NOT_FOUND if unavailable.
int32_t __luna_process_executable(const uint8_t** out_ptr, size_t* out_len);

// Low-level CRT accessors (internal runtime / test use only)
int    __luna_argc(void);
char** __luna_argv(void);

#ifdef __cplusplus
} // extern "C"
#endif
