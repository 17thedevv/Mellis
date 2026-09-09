// =============================================================================
// mellis/runtime/io.h
//
// Mellis Runtime Primitive Stdio ABI (FROZEN)
// =============================================================================

#pragma once

#include "abi.h"

#ifdef __cplusplus
extern "C" {
#endif

void __mellis_print(const uint8_t* str, size_t len);
void __mellis_println(const uint8_t* str, size_t len);
void __mellis_eprintln(const uint8_t* str, size_t len);

#ifdef __cplusplus
}
#endif
