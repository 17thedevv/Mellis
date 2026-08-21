#include "mellis/runtime/io.h"
#include <stdio.h>

void __mellis_print(const uint8_t* str, uint64_t len) {
    if (str && len) fwrite(str, 1, (size_t)len, stdout);
}

void __mellis_println(const uint8_t* str, uint64_t len) {
    __mellis_print(str, len);
    fputc('\n', stdout);
}
