// =============================================================================
// runtime/src/panic/panic.c
//
// Mellis Runtime — Panic / Trap (Hosted Default Implementation)
//
// Default behavior: print a diagnostic to stderr and abort().
// In release builds, the message may be suppressed for size.
// In freestanding/embedded, replace this with a minimal trap.
// =============================================================================

#include "mellis/runtime/panic.h"
#include <stdio.h>
#include <stdlib.h>

// --- Primary Panic -----------------------------------------------------------

MELLIS_NORETURN void __mellis_panic(const MellisPanicInfo* info) {
    if (info) {
        const char* file = info->file ? info->file : "<unknown>";
        const char* msg  = (info->message && info->message_len > 0)
                           ? info->message : "<no message>";
        fprintf(stderr,
            "\nmellis: PANIC [M%03u] %s\n"
            "    at %s:%u:%u\n",
            (unsigned)info->error_code, msg,
            file, (unsigned)info->line, (unsigned)info->column);
    } else {
        fprintf(stderr, "\nmellis: PANIC (no info)\n");
    }
    abort();
}

// --- Bounds Failure ----------------------------------------------------------

MELLIS_NORETURN void __mellis_bounds_fail(size_t idx, size_t len,
                                          const char* file, uint32_t line) {
    fprintf(stderr,
        "\nmellis: PANIC [M%03u] index out of bounds: index %zu, length %zu\n"
        "    at %s:%u\n",
        MELLIS_ERR_BOUNDS_VIOLATION, idx, len,
        file ? file : "<unknown>", (unsigned)line);
    abort();
}

// --- Division by Zero --------------------------------------------------------

MELLIS_NORETURN void __mellis_div_zero_fail(const char* file, uint32_t line) {
    fprintf(stderr,
        "\nmellis: PANIC [M%03u] division by zero\n"
        "    at %s:%u\n",
        MELLIS_ERR_DIV_ZERO,
        file ? file : "<unknown>", (unsigned)line);
    abort();
}

// --- Assertion Failure -------------------------------------------------------

MELLIS_NORETURN void __mellis_assert_fail(const char* msg,
                                          const char* file, uint32_t line) {
    fprintf(stderr,
        "\nmellis: PANIC [M%03u] assertion failed: %s\n"
        "    at %s:%u\n",
        MELLIS_ERR_ASSERT_FAILURE,
        msg ? msg : "<no message>",
        file ? file : "<unknown>", (unsigned)line);
    abort();
}

// --- Overflow Failure --------------------------------------------------------

MELLIS_NORETURN void __mellis_overflow_fail(const char* file, uint32_t line) {
    fprintf(stderr,
        "\nmellis: PANIC [M%03u] integer overflow\n"
        "    at %s:%u\n",
        5, // future stable code
        file ? file : "<unknown>", (unsigned)line);
    abort();
}
