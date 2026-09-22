// =============================================================================
// runtime/src/memory/memory.c
//
// Luna Runtime — Memory Allocation (Hosted Default Implementation)
// =============================================================================

#include "luna/runtime/memory.h"
#include "luna/runtime/panic.h"
#include <stdlib.h>
#include <string.h>

// Shared, runtime-owned zero-size sentinel aligned to LUNA_MAX_ALIGN (4096).
static _Alignas(LUNA_MAX_ALIGN) const unsigned char g_luna_zero_sentinel[1] = { 0 };

int __luna_is_zero_sentinel(const void* ptr) {
    return ptr == (const void*)g_luna_zero_sentinel;
}

static inline int is_valid_align(size_t align) {
    return align > 0 && align <= LUNA_MAX_ALIGN && (align & (align - 1)) == 0;
}

int __luna_runtime_abi_version(void) {
    return LUNA_RUNTIME_ABI_VERSION;
}

// --- Allocation ---------------------------------------------------------------

void* __luna_alloc(size_t size, size_t align) {
    if (!is_valid_align(align)) {
        static const uint8_t msg[] = "Invalid alignment in __luna_alloc";
        __luna_panic_code(LUNA_ERR_INVALID_STATE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

    if (size == 0) {
        return (void*)g_luna_zero_sentinel;
    }

    // Guard against size_t overflow when aligning size
    if (size > SIZE_MAX - (align - 1)) {
        static const uint8_t msg[] = "Allocation size overflow in __luna_alloc";
        __luna_panic_code(LUNA_ERR_ALLOC_FAILURE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
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
        static const uint8_t msg[] = "Out of memory in __luna_alloc";
        __luna_panic_code(LUNA_ERR_ALLOC_FAILURE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

    return ptr;
}

// --- Deallocation ------------------------------------------------------------

void __luna_dealloc(void* ptr, size_t size, size_t align) {
    (void)size;
    if (!ptr || __luna_is_zero_sentinel(ptr)) {
        return;
    }

    if (!is_valid_align(align)) {
        static const uint8_t msg[] = "Invalid alignment in __luna_dealloc";
        __luna_panic_code(LUNA_ERR_INVALID_STATE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

#if defined(_WIN32)
    _aligned_free(ptr);
#else
    free(ptr);
#endif
}

// --- Reallocation ------------------------------------------------------------

void* __luna_realloc(void* ptr, size_t old_size, size_t old_align, size_t new_size) {
    if (!ptr || __luna_is_zero_sentinel(ptr)) {
        return __luna_alloc(new_size, old_align);
    }

    if (new_size == 0) {
        __luna_dealloc(ptr, old_size, old_align);
        return (void*)g_luna_zero_sentinel;
    }

    if (!is_valid_align(old_align)) {
        static const uint8_t msg[] = "Invalid alignment in __luna_realloc";
        __luna_panic_code(LUNA_ERR_INVALID_STATE, msg, sizeof(msg) - 1, NULL, 0, 0, 0);
    }

    // Allocate new block with same alignment guarantee
    void* new_ptr = __luna_alloc(new_size, old_align);
    size_t copy_size = old_size < new_size ? old_size : new_size;
    if (copy_size > 0) {
        memcpy(new_ptr, ptr, copy_size);
    }
    __luna_dealloc(ptr, old_size, old_align);
    return new_ptr;
}

// --- Optional Fallbacks ------------------------------------------------------

void* __luna_mem_copy(void* dst, const void* src, size_t n) {
    return memcpy(dst, src, n);
}

void* __luna_mem_move(void* dst, const void* src, size_t n) {
    return memmove(dst, src, n);
}

void* __luna_mem_set(void* dst, int val, size_t n) {
    return memset(dst, val, n);
}
