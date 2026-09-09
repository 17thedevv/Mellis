// =============================================================================
// runtime/src/panic/panic.c
//
// Mellis Runtime — Panic / Trap (Hosted Default Implementation)
// =============================================================================

#include "mellis/runtime/panic.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// --- Primary Panic Implementation --------------------------------------------

MELLIS_NORETURN void __mellis_panic_code(
    uint32_t       error_code,
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
) {
    const char* msg = (msg_ptr && msg_len > 0) ? (const char*)msg_ptr : "<no message>";
    int mlen = (msg_ptr && msg_len > 0) ? (int)msg_len : 12;
    const char* file = (file_ptr && file_len > 0) ? (const char*)file_ptr : "<unknown>";
    int flen = (file_ptr && file_len > 0) ? (int)file_len : 9;

    fprintf(stderr,
        "\nmellis: PANIC [M%03u] %.*s\n"
        "    at %.*s:%u:%u\n",
        (unsigned)error_code, mlen, msg,
        flen, file, (unsigned)line, (unsigned)col);
    fflush(stderr);
    abort();
}

MELLIS_NORETURN void __mellis_panic(
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
) {
    __mellis_panic_code(MELLIS_ERR_INVALID_STATE, msg_ptr, msg_len, file_ptr, file_len, line, col);
}

// --- Bounds Failure ----------------------------------------------------------

MELLIS_NORETURN void __mellis_bounds_fail(
    size_t         index,
    size_t         len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
) {
    const char* file = (file_ptr && file_len > 0) ? (const char*)file_ptr : "<unknown>";
    int flen = (file_ptr && file_len > 0) ? (int)file_len : 9;

    fprintf(stderr,
        "\nmellis: PANIC [M%03u] index out of bounds: index %zu, length %zu\n"
        "    at %.*s:%u:%u\n",
        MELLIS_ERR_BOUNDS_VIOLATION, index, len,
        flen, file, (unsigned)line, (unsigned)col);
    fflush(stderr);
    abort();
}

// --- Compatibility Traps -----------------------------------------------------

MELLIS_NORETURN void __mellis_div_zero_fail(const char* file, uint32_t line) {
    static const uint8_t msg[] = "division by zero";
    __mellis_panic_code(
        MELLIS_ERR_DIV_ZERO,
        msg, sizeof(msg) - 1,
        (const uint8_t*)file, file ? strlen(file) : 0,
        line, 0
    );
}

MELLIS_NORETURN void __mellis_assert_fail(const char* msg, const char* file, uint32_t line) {
    __mellis_panic_code(
        MELLIS_ERR_ASSERT_FAILURE,
        (const uint8_t*)msg, msg ? strlen(msg) : 0,
        (const uint8_t*)file, file ? strlen(file) : 0,
        line, 0
    );
}

MELLIS_NORETURN void __mellis_overflow_fail(const char* file, uint32_t line) {
    static const uint8_t msg[] = "integer overflow";
    __mellis_panic_code(
        MELLIS_ERR_INVALID_STATE,
        msg, sizeof(msg) - 1,
        (const uint8_t*)file, file ? strlen(file) : 0,
        line, 0
    );
}
