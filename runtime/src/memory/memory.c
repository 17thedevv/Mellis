// =============================================================================
// runtime/src/memory/memory.c
//
// Mellis Runtime — Memory Allocation (Hosted Default Implementation)
// =============================================================================

#include "mellis/runtime/memory.h"
#include "mellis/runtime/panic.h"
#include <stdlib.h>
#include <string.h>

// Shared, runtime-owned zero-size sentinel aligned to MELLIS_MAX_ALIGN (4096).
static _Alignas(MELLIS_MAX_ALIGN) const unsigned char g_mellis_zero_sentinel[1] = { 0 };

int __mellis_is_zero_sentinel(const void* ptr) {
    return ptr == (const void*)g_mellis_zero_sentinel;
}

static inline int is_valid_align(size_t align) {
    return align > 0 && align <= MELLIS_MAX_ALIGN && (align & (align - 1)) == 0;
}

// --- Allocation ---------------------------------------------------------------

void* __mellis_alloc(size_t size, size_t align) {
    if (!is_valid_align(align)) {
        static const uint8_t msg[] = "Invalid alignment in __mellis_alloc";
        __mellis_panic_code(MELLIS_ERR_INVALID_STATE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

    if (size == 0) {
        return (void*)g_mellis_zero_sentinel;
    }

    // Guard against size_t overflow when aligning size
    if (size > SIZE_MAX - (align - 1)) {
        static const uint8_t msg[] = "Allocation size overflow in __mellis_alloc";
        __mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

    void* ptr = NULL;
#if defined(_WIN32)
    ptr = _aligned_malloc(size, align);
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
    size_t aligned_size = (size + align - 1) & ~(align - 1);
    ptr = aligned_alloc(align, aligned_size);
#else
    if (posix_memalign(&ptr, align, size) != 0) {
        ptr = NULL;
    }
#endif

    if (!ptr) {
        static const uint8_t msg[] = "Out of memory in __mellis_alloc";
        __mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

    return ptr;
}

// --- Deallocation ------------------------------------------------------------

void __mellis_dealloc(void* ptr, size_t size, size_t align) {
    (void)size;
    if (!ptr || __mellis_is_zero_sentinel(ptr)) {
        return;
    }

    if (!is_valid_align(align)) {
        static const uint8_t msg[] = "Invalid alignment in __mellis_dealloc";
        __mellis_panic_code(MELLIS_ERR_INVALID_STATE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

#if defined(_WIN32)
    _aligned_free(ptr);
#else
    free(ptr);
#endif
}

void __mellis_free(void* ptr, size_t size, size_t align) {
    __mellis_dealloc(ptr, size, align);
}

// --- Reallocation ------------------------------------------------------------

void* __mellis_realloc(void* ptr, size_t old_size, size_t old_align, size_t new_size) {
    if (!ptr || __mellis_is_zero_sentinel(ptr)) {
        return __mellis_alloc(new_size, old_align);
    }

    if (new_size == 0) {
        __mellis_dealloc(ptr, old_size, old_align);
        return (void*)g_mellis_zero_sentinel;
    }

    if (!is_valid_align(old_align)) {
        static const uint8_t msg[] = "Invalid alignment in __mellis_realloc";
        __mellis_panic_code(MELLIS_ERR_INVALID_STATE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

    // Allocate new block with same alignment guarantee
    void* new_ptr = __mellis_alloc(new_size, old_align);
    size_t copy_size = old_size < new_size ? old_size : new_size;
    if (copy_size > 0) {
        memcpy(new_ptr, ptr, copy_size);
    }
    __mellis_dealloc(ptr, old_size, old_align);
    return new_ptr;
}

// --- Optional Fallbacks ------------------------------------------------------

void* __mellis_mem_copy(void* dst, const void* src, size_t n) {
    return memcpy(dst, src, n);
}

void* __mellis_mem_move(void* dst, const void* src, size_t n) {
    return memmove(dst, src, n);
}

void* __mellis_mem_set(void* dst, int val, size_t n) {
    return memset(dst, val, n);
}
