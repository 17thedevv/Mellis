// =============================================================================
// luna/runtime/io.h
//
// Luna Runtime Primitive Stdio ABI (FROZEN)
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

void __luna_print(const uint8_t* str, size_t len);
void __luna_println(const uint8_t* str, size_t len);
void __luna_eprintln(const uint8_t* str, size_t len);

// --- Whole-File I/O (ABI v1) -------------------------------------------------

// Reads an entire file into a newly allocated buffer (allocated via __luna_alloc).
// Rejects paths with embedded NUL (LUNA_STATUS_INVALID_ARGUMENT).
// On success:
//   *out_ptr receives buffer
//   *out_len receives actual bytes read
//   *out_cap receives allocated capacity (*out_len <= *out_cap)
//   returns LUNA_STATUS_OK.
// Ownership is transferred to caller; free with __luna_buffer_free(*out_ptr, *out_cap).
int32_t __luna_read_file(
    const uint8_t*  path_ptr,
    size_t          path_len,
    uint8_t**       out_ptr,
    size_t*         out_len,
    size_t*         out_cap
);

// Writes data_len bytes to path. Creates or truncates the file.
// Rejects paths with embedded NUL (LUNA_STATUS_INVALID_ARGUMENT).
// Returns LUNA_STATUS_OK on success, or a negative LunaStatus error code.
int32_t __luna_write_file(
    const uint8_t*  path_ptr,
    size_t          path_len,
    const uint8_t*  data_ptr,
    size_t          data_len
);

// --- Stdin Line Input (ABI v1) -----------------------------------------------

// Reads a single line from standard input up to delimiter '\n' or EOF.
// Strips trailing '\n' and '\r' (CRLF normalization).
// Returns:
//   LUNA_STATUS_OK: line read (out_len is length; out_cap is buffer capacity).
//   LUNA_STATUS_EOF: EOF reached before any characters read (*out_ptr = NULL, *out_len = 0).
//   Negative LunaStatus on error.
int32_t __luna_read_line(
    uint8_t**       out_ptr,
    size_t*         out_len,
    size_t*         out_cap
);

// Release a runtime-allocated I/O buffer with known capacity
void __luna_buffer_free(uint8_t* ptr, size_t cap);

#ifdef __cplusplus
}
#endif
