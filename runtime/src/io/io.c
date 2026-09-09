// =============================================================================
// runtime/src/io/io.c
//
// Mellis Runtime — Primitive Stdio (Hosted Default Implementation)
// =============================================================================

#include "mellis/runtime/io.h"
#include <stdio.h>

void __mellis_print(const uint8_t* str, size_t len) {
    if (str && len) {
        fwrite(str, 1, len, stdout);
        fflush(stdout);
    }
}

void __mellis_println(const uint8_t* str, size_t len) {
    if (str && len) {
        fwrite(str, 1, len, stdout);
    }
    fputc('\n', stdout);
    fflush(stdout);
}

void __mellis_eprintln(const uint8_t* str, size_t len) {
    if (str && len) {
        fwrite(str, 1, len, stderr);
    }
    fputc('\n', stderr);
    fflush(stderr);
}
