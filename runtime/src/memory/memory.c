// =============================================================================
// runtime/src/memory/memory.c
//
// Mellis Runtime — Memory Allocation (Hosted Default Implementation)
//
// This is the hosted profile implementation. Maps to libc malloc/free.
// Custom allocators and embedded profiles provide their own implementations
// by replacing this translation unit.
// =============================================================================

#include "mellis/runtime/memory.h"
#include "mellis/runtime/panic.h"
#include <stdlib.h>
#include <string.h>

// --- Allocation ---------------------------------------------------------------

void* __mellis_alloc(size_t size, size_t align) {
    if (size == 0) {
        // Return a unique non-null dangling pointer for zero-size allocations.
        return (void*)1;
    }
#if defined(_WIN32)
    return _aligned_malloc(size, align);
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
    // C11 aligned_alloc requires size to be a multiple of align.
    size_t aligned_size = (size + align - 1) & ~(align - 1);
    return aligned_alloc(align, aligned_size);
#else
    // Fallback: posix_memalign
    void* ptr = NULL;
    if (posix_memalign(&ptr, align, size) != 0) return NULL;
    return ptr;
#endif
}

// --- Deallocation ------------------------------------------------------------

void __mellis_dealloc(void* ptr, size_t size, size_t align) {
    (void)size;  // Ignored by hosted allocator; useful for custom allocators.
    (void)align;
    if (!ptr || (size_t)ptr == 1) return; // null or zero-size sentinel
#if defined(_WIN32)
    _aligned_free(ptr);
#else
    free(ptr);
#endif
}

// --- Reallocation ------------------------------------------------------------

void* __mellis_realloc(void* ptr, size_t old_size, size_t new_size, size_t align) {
    (void)old_size;
    if (new_size == 0) {
        __mellis_dealloc(ptr, old_size, align);
        return (void*)1; // zero-size sentinel
    }
    // Slow path: allocate new, copy, free old (to preserve alignment guarantee).
    void* new_ptr = __mellis_alloc(new_size, align);
    if (!new_ptr) return NULL;
    if (ptr && (size_t)ptr != 1) {
        size_t copy_size = old_size < new_size ? old_size : new_size;
        memcpy(new_ptr, ptr, copy_size);
        __mellis_dealloc(ptr, old_size, align);
    }
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
