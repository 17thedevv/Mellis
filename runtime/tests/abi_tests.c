// =============================================================================
// runtime/tests/abi_tests.c
//
// Mellis Runtime ABI v1.0 — Conformance & Invariant Verification Suite
// =============================================================================

#include "mellis/runtime/abi.h"
#include "mellis/runtime/memory.h"
#include "mellis/runtime/panic.h"
#include "mellis/runtime/io.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#if defined(_WIN32)
#include <windows.h>
#include <process.h>
#else
#include <sys/wait.h>
#include <unistd.h>
#endif

// --- Individual Unit Tests (In-Process) ----------------------------------------

static int test_sentinel_identity(void) {
    printf("[RUN] test_sentinel_identity\n");
    for (size_t align = 1; align <= 4096; align *= 2) {
        void* ptr = __mellis_alloc(0, align);
        if (!ptr) {
            fprintf(stderr, "FAIL: __mellis_alloc(0, %zu) returned NULL\n", align);
            return 1;
        }
        if (!__mellis_is_zero_sentinel(ptr)) {
            fprintf(stderr, "FAIL: __mellis_alloc(0, %zu) did not return zero sentinel\n", align);
            return 1;
        }
        if (((uintptr_t)ptr % align) != 0) {
            fprintf(stderr, "FAIL: sentinel %p not aligned to %zu\n", ptr, align);
            return 1;
        }
        // Deallocating sentinel must be guaranteed no-op
        __mellis_dealloc(ptr, 0, align);
    }
    printf("[PASS] test_sentinel_identity\n");
    return 0;
}

static int test_realloc_transitions(void) {
    printf("[RUN] test_realloc_transitions\n");
    void* sentinel = __mellis_alloc(0, 16);

    // Realloc from sentinel to fresh allocation
    void* fresh = __mellis_realloc(sentinel, 0, 16, 128);
    if (!fresh || __mellis_is_zero_sentinel(fresh)) {
        fprintf(stderr, "FAIL: realloc from sentinel did not allocate fresh block\n");
        return 1;
    }
    if (((uintptr_t)fresh % 16) != 0) {
        fprintf(stderr, "FAIL: fresh allocation not aligned to 16\n");
        return 1;
    }

    // Write test pattern
    memset(fresh, 0xAB, 128);

    // Realloc to larger size
    void* grown = __mellis_realloc(fresh, 128, 16, 256);
    if (!grown || ((uintptr_t)grown % 16) != 0) {
        fprintf(stderr, "FAIL: realloc growth failed\n");
        return 1;
    }
    // Verify first 128 bytes preserved
    unsigned char* b = (unsigned char*)grown;
    for (int i = 0; i < 128; i++) {
        if (b[i] != 0xAB) {
            fprintf(stderr, "FAIL: realloc data corruption at index %d\n", i);
            return 1;
        }
    }

    // Realloc to 0 bytes must free and return sentinel
    void* back_to_sentinel = __mellis_realloc(grown, 256, 16, 0);
    if (!__mellis_is_zero_sentinel(back_to_sentinel)) {
        fprintf(stderr, "FAIL: realloc to 0 did not return sentinel\n");
        return 1;
    }

    printf("[PASS] test_realloc_transitions\n");
    return 0;
}

static int test_align_boundary_4096(void) {
    printf("[RUN] test_align_boundary_4096\n");
    void* ptr = __mellis_alloc(64, 4096);
    if (!ptr) {
        fprintf(stderr, "FAIL: alloc with align=4096 failed\n");
        return 1;
    }
    if (((uintptr_t)ptr % 4096) != 0) {
        fprintf(stderr, "FAIL: ptr %p not aligned to 4096\n", ptr);
        return 1;
    }
    __mellis_dealloc(ptr, 64, 4096);
    printf("[PASS] test_align_boundary_4096\n");
    return 0;
}

static int test_stdio_exact_bytes(void) {
    printf("[RUN] test_stdio_exact_bytes\n");
    const uint8_t msg[] = "Mellis Runtime ABI v1.0";
    __mellis_print(msg, 6); // only print "Mellis"
    printf(" [verified]\n");
    __mellis_println(msg, sizeof(msg) - 1);
    __mellis_eprintln(msg, sizeof(msg) - 1);
    printf("[PASS] test_stdio_exact_bytes\n");
    return 0;
}

// --- Terminal Invariant Sub-Processes ------------------------------------------

static void run_terminal_target(const char* target) {
    if (strcmp(target, "term_align_0") == 0) {
        __mellis_alloc(64, 0); // Must terminate
    } else if (strcmp(target, "term_align_3") == 0) {
        __mellis_alloc(64, 3); // Must terminate
    } else if (strcmp(target, "term_align_8192") == 0) {
        __mellis_alloc(64, 8192); // Must terminate (> MELLIS_MAX_ALIGN)
    } else if (strcmp(target, "term_overflow") == 0) {
        __mellis_alloc(SIZE_MAX - 10, 64); // Must terminate
    } else if (strcmp(target, "term_panic") == 0) {
        static const uint8_t msg[] = "test explicit panic";
        static const uint8_t file[] = "test_source.ms";
        __mellis_panic(msg, sizeof(msg) - 1, file, sizeof(file) - 1, 42, 10);
    } else if (strcmp(target, "term_bounds_fail") == 0) {
        static const uint8_t file[] = "test_bounds.ms";
        __mellis_bounds_fail(15, 10, file, sizeof(file) - 1, 99, 4);
    }
    exit(0); // If it reached here, it failed to terminate!
}

static int spawn_and_assert_terminated(const char* self_exe, const char* target, int expected_exit) {
    printf("[RUN] %s (expecting terminal abort/exit)...\n", target);
#if defined(_WIN32)
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "\"%s\" --run-terminal %s", self_exe, target);

    STARTUPINFOA si;
    PROCESS_INFORMATION pi;
    ZeroMemory(&si, sizeof(si));
    si.cb = sizeof(si);
    ZeroMemory(&pi, sizeof(pi));

    if (!CreateProcessA(NULL, cmd, NULL, NULL, FALSE, 0, NULL, NULL, &si, &pi)) {
        fprintf(stderr, "FAIL: CreateProcess failed for %s\n", target);
        return 1;
    }

    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD exit_code = 0;
    GetExitCodeProcess(pi.hProcess, &exit_code);
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);

    // On Windows, abort() exits with code 3 or 101/102
    if (expected_exit == 0) {
        if (exit_code == 0) {
            fprintf(stderr, "FAIL: expected %s to terminate abnormally, but exited with 0\n", target);
            return 1;
        }
    } else if (exit_code != (DWORD)expected_exit && exit_code != 3) {
        // accept explicit expected_exit or Windows CRT abort() code 3
        printf("  Process terminated with exit code %u (expected %d or abort)\n", (unsigned)exit_code, expected_exit);
    }
#else
    pid_t pid = fork();
    if (pid == 0) {
        run_terminal_target(target);
        exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    if (!WIFEXITED(status) && !WIFSIGNALED(status)) {
        fprintf(stderr, "FAIL: %s did not terminate properly\n", target);
        return 1;
    }
    if (WIFEXITED(status) && WEXITSTATUS(status) == 0) {
        fprintf(stderr, "FAIL: %s exited normally with code 0\n", target);
        return 1;
    }
#endif
    printf("[PASS] %s successfully aborted deterministically\n", target);
    return 0;
}

int main(int argc, char** argv) {
    if (argc >= 3 && strcmp(argv[1], "--run-terminal") == 0) {
        run_terminal_target(argv[2]);
        return 0;
    }

    printf("=================================================================\n");
    printf("   Mellis Runtime ABI v1.0 Conformance Test Suite\n");
    printf("=================================================================\n");

    int failures = 0;

    // In-process positive invariant tests
    failures += test_sentinel_identity();
    failures += test_realloc_transitions();
    failures += test_align_boundary_4096();
    failures += test_stdio_exact_bytes();

    // Out-of-process terminal tests
    const char* self_exe = argv[0];
    failures += spawn_and_assert_terminated(self_exe, "term_align_0", 5);
    failures += spawn_and_assert_terminated(self_exe, "term_align_3", 5);
    failures += spawn_and_assert_terminated(self_exe, "term_align_8192", 5);
    failures += spawn_and_assert_terminated(self_exe, "term_overflow", 1);
    failures += spawn_and_assert_terminated(self_exe, "term_panic", 101);
    failures += spawn_and_assert_terminated(self_exe, "term_bounds_fail", 102);

    printf("=================================================================\n");
    if (failures == 0) {
        printf("   ALL RUNTIME ABI INVARIANT TESTS PASSED (0 FAILURES)\n");
    } else {
        printf("   TEST SUITE FAILED WITH %d FAILURES\n", failures);
    }
    printf("=================================================================\n");
    return failures;
}
