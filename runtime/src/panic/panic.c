// =============================================================================
// runtime/src/panic/panic.c
//
// Luna Runtime — Panic / Trap (Hosted Default Implementation)
// =============================================================================

#include "luna/runtime/panic.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// --- Primary Panic Implementation --------------------------------------------

LUNA_NORETURN void __luna_panic_code(
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
        "\nluna: PANIC [L%03u] %.*s\n"
        "    at %.*s:%u:%u\n",
        (unsigned)error_code, mlen, msg,
        flen, file, (unsigned)line, (unsigned)col);
    fflush(stderr);
    abort();
}

LUNA_NORETURN void __luna_panic(
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
) {
    __luna_panic_code(LUNA_ERR_INVALID_STATE, msg_ptr, msg_len, file_ptr, file_len, line, col);
}

LUNA_NORETURN void __luna_panic_default(void) {
    static const uint8_t msg[] = "explicit panic";
    __luna_panic_code(LUNA_ERR_INVALID_STATE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
}

// --- Bounds Failure ----------------------------------------------------------

LUNA_NORETURN void __luna_bounds_fail(
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
        "\nluna: PANIC [L%03u] index out of bounds: index %zu, length %zu\n"
        "    at %.*s:%u:%u\n",
        LUNA_ERR_BOUNDS_VIOLATION, index, len,
        flen, file, (unsigned)line, (unsigned)col);
    fflush(stderr);
    abort();
}

// --- Condition Traps ---------------------------------------------------------

LUNA_NORETURN void __luna_div_zero_fail(const char* file, uint32_t line) {
    static const uint8_t msg[] = "division by zero";
    __luna_panic_code(
        LUNA_ERR_DIV_ZERO,
        msg, sizeof(msg) - 1,
        (const uint8_t*)file, file ? strlen(file) : 0,
        line, 0
    );
}

LUNA_NORETURN void __luna_assert_fail(const char* msg, const char* file, uint32_t line) {
    __luna_panic_code(
        LUNA_ERR_ASSERT_FAILURE,
        (const uint8_t*)msg, msg ? strlen(msg) : 0,
        (const uint8_t*)file, file ? strlen(file) : 0,
        line, 0
    );
}

LUNA_NORETURN void __luna_overflow_fail(const char* file, uint32_t line) {
    static const uint8_t msg[] = "integer overflow";
    __luna_panic_code(
        LUNA_ERR_INVALID_STATE,
        msg, sizeof(msg) - 1,
        (const uint8_t*)file, file ? strlen(file) : 0,
        line, 0
    );
}
