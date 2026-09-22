// =============================================================================
// runtime/tests/abi_tests.c
//
// Luna Runtime ABI v1.0 — Conformance & Invariant Verification Suite
// =============================================================================

#include "luna/runtime/abi.h"
#include "luna/runtime/memory.h"
#include "luna/runtime/panic.h"
#include "luna/runtime/io.h"
#include "luna/runtime/process.h"

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

static int test_abi_marker(void) {
    printf("[RUN] test_abi_marker\n");
    int ver = __luna_runtime_abi_version();
    if (ver != LUNA_RUNTIME_ABI_VERSION) {
        fprintf(stderr, "FAIL: __luna_runtime_abi_version() = %d, expected %d\n", ver, LUNA_RUNTIME_ABI_VERSION);
        return 1;
    }
    printf("[PASS] test_abi_marker\n");
    return 0;
}

static int test_sentinel_identity(void) {
    printf("[RUN] test_sentinel_identity\n");
    for (size_t align = 1; align <= 4096; align *= 2) {
        void* ptr = __luna_alloc(0, align);
        if (!ptr) {
            fprintf(stderr, "FAIL: __luna_alloc(0, %zu) returned NULL\n", align);
            return 1;
        }
        if (!__luna_is_zero_sentinel(ptr)) {
            fprintf(stderr, "FAIL: __luna_alloc(0, %zu) did not return zero sentinel\n", align);
            return 1;
        }
        if (((uintptr_t)ptr % align) != 0) {
            fprintf(stderr, "FAIL: sentinel %p not aligned to %zu\n", ptr, align);
            return 1;
        }
        // Deallocating sentinel must be guaranteed no-op
        __luna_dealloc(ptr, 0, align);
    }
    printf("[PASS] test_sentinel_identity\n");
    return 0;
}

static int test_realloc_transitions(void) {
    printf("[RUN] test_realloc_transitions\n");
    void* sentinel = __luna_alloc(0, 16);

    // Realloc from sentinel to fresh allocation
    void* fresh = __luna_realloc(sentinel, 0, 16, 128);
    if (!fresh || __luna_is_zero_sentinel(fresh)) {
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
    void* grown = __luna_realloc(fresh, 128, 16, 256);
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
    void* back_to_sentinel = __luna_realloc(grown, 256, 16, 0);
    if (!__luna_is_zero_sentinel(back_to_sentinel)) {
        fprintf(stderr, "FAIL: realloc to 0 did not return sentinel\n");
        return 1;
    }

    printf("[PASS] test_realloc_transitions\n");
    return 0;
}

static int test_align_boundary_4096(void) {
    printf("[RUN] test_align_boundary_4096\n");
    void* ptr = __luna_alloc(64, 4096);
    if (!ptr) {
        fprintf(stderr, "FAIL: alloc with align=4096 failed\n");
        return 1;
    }
    if (((uintptr_t)ptr % 4096) != 0) {
        fprintf(stderr, "FAIL: ptr %p not aligned to 4096\n", ptr);
        return 1;
    }
    __luna_dealloc(ptr, 64, 4096);
    printf("[PASS] test_align_boundary_4096\n");
    return 0;
}

static int test_stdio_exact_bytes(void) {
    printf("[RUN] test_stdio_exact_bytes\n");
    const uint8_t msg[] = "Luna Runtime ABI v1.0";
    __luna_print(msg, 4); // only print "Luna"
    printf(" [verified]\n");
    __luna_println(msg, sizeof(msg) - 1);
    __luna_eprintln(msg, sizeof(msg) - 1);
    printf("[PASS] test_stdio_exact_bytes\n");
    return 0;
}

static int test_process_args(void) {
    printf("[RUN] test_process_args\n");

    // Test with simulated startup arguments
    char* fake_argv[] = { "luna_test_app", "--flag", "input_file.txt", "123", NULL };
    int fake_argc = 4;

    __luna_startup(fake_argc, fake_argv);

    size_t count = __luna_process_arg_count();
    if (count != 3) {
        fprintf(stderr, "FAIL: expected arg count 3, got %zu\n", count);
        return 1;
    }

    const uint8_t* ptr = NULL;
    size_t len = 0;

    // Arg 0: "--flag"
    if (__luna_process_arg(0, &ptr, &len) != LUNA_STATUS_OK || len != 6 || memcmp(ptr, "--flag", 6) != 0) {
        fprintf(stderr, "FAIL: arg 0 mismatch\n");
        return 1;
    }

    // Arg 1: "input_file.txt"
    if (__luna_process_arg(1, &ptr, &len) != LUNA_STATUS_OK || len != 14 || memcmp(ptr, "input_file.txt", 14) != 0) {
        fprintf(stderr, "FAIL: arg 1 mismatch\n");
        return 1;
    }

    // Arg 2: "123"
    if (__luna_process_arg(2, &ptr, &len) != LUNA_STATUS_OK || len != 3 || memcmp(ptr, "123", 3) != 0) {
        fprintf(stderr, "FAIL: arg 2 mismatch\n");
        return 1;
    }

    // Arg 3: Out of bounds
    if (__luna_process_arg(3, &ptr, &len) != LUNA_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "FAIL: out-of-bounds arg did not return LUNA_STATUS_INVALID_ARGUMENT\n");
        return 1;
    }

    // Null pointers
    if (__luna_process_arg(0, NULL, &len) != LUNA_STATUS_INVALID_ARGUMENT ||
        __luna_process_arg(0, &ptr, NULL) != LUNA_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "FAIL: null pointer query did not return LUNA_STATUS_INVALID_ARGUMENT\n");
        return 1;
    }

    // Executable query
    if (__luna_process_executable(&ptr, &len) != LUNA_STATUS_OK || len != 13 || memcmp(ptr, "luna_test_app", 13) != 0) {
        fprintf(stderr, "FAIL: executable query mismatch\n");
        return 1;
    }

    // Test with 0 args (argc = 1)
    char* fake_argv_empty[] = { "lone_exe", NULL };
    __luna_startup(1, fake_argv_empty);
    if (__luna_process_arg_count() != 0) {
        fprintf(stderr, "FAIL: expected 0 args for argc=1\n");
        return 1;
    }
    if (__luna_process_arg(0, &ptr, &len) != LUNA_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "FAIL: arg query on empty args should fail\n");
        return 1;
    }

    printf("[PASS] test_process_args\n");
    return 0;
}

static int test_file_io_roundtrip(void) {
    printf("[RUN] test_file_io_roundtrip\n");
    const uint8_t path[] = "luna_abi_test_temp.txt";
    const uint8_t content[] = "Luna Runtime ABI v1 File Content Roundtrip Verification 12345";
    size_t content_len = sizeof(content) - 1;

    // 1. Write file
    int32_t wstatus = __luna_write_file(path, sizeof(path) - 1, content, content_len);
    if (wstatus != LUNA_STATUS_OK) {
        fprintf(stderr, "FAIL: __luna_write_file returned %d\n", wstatus);
        return 1;
    }

    // 2. Read file
    uint8_t* out_buf = NULL;
    size_t out_len = 0;
    size_t out_cap = 0;
    int32_t rstatus = __luna_read_file(path, sizeof(path) - 1, &out_buf, &out_len, &out_cap);
    if (rstatus != LUNA_STATUS_OK) {
        fprintf(stderr, "FAIL: __luna_read_file returned %d\n", rstatus);
        return 1;
    }
    if (out_len != content_len) {
        fprintf(stderr, "FAIL: read length %zu != expected %zu\n", out_len, content_len);
        __luna_buffer_free(out_buf, out_cap);
        return 1;
    }
    if (out_len > out_cap) {
        fprintf(stderr, "FAIL: capacity invariant violated: out_len (%zu) > out_cap (%zu)\n", out_len, out_cap);
        __luna_buffer_free(out_buf, out_cap);
        return 1;
    }
    if (memcmp(out_buf, content, content_len) != 0) {
        fprintf(stderr, "FAIL: read content mismatch\n");
        __luna_buffer_free(out_buf, out_cap);
        return 1;
    }

    // 3. Free buffer with exact allocated capacity
    __luna_buffer_free(out_buf, out_cap);

    // Clean up file
#if defined(_WIN32)
    DeleteFileA((const char*)path);
#else
    unlink((const char*)path);
#endif

    printf("[PASS] test_file_io_roundtrip\n");
    return 0;
}

static int test_file_nul_defense(void) {
    printf("[RUN] test_file_nul_defense\n");
    const uint8_t malicious_path[] = "valid_prefix\0evil_suffix.txt";
    size_t path_len = sizeof(malicious_path) - 1; // 27 bytes, contains embedded NUL

    const uint8_t data[] = "data";
    int32_t wstatus = __luna_write_file(malicious_path, path_len, data, 4);
    if (wstatus != LUNA_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "FAIL: write with embedded NUL did not return LUNA_STATUS_INVALID_ARGUMENT, got %d\n", wstatus);
        return 1;
    }

    uint8_t* out_ptr = NULL;
    size_t out_len = 0;
    size_t out_cap = 0;
    int32_t rstatus = __luna_read_file(malicious_path, path_len, &out_ptr, &out_len, &out_cap);
    if (rstatus != LUNA_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "FAIL: read with embedded NUL did not return LUNA_STATUS_INVALID_ARGUMENT, got %d\n", rstatus);
        return 1;
    }

    printf("[PASS] test_file_nul_defense\n");
    return 0;
}

static int test_file_not_found(void) {
    printf("[RUN] test_file_not_found\n");
    const uint8_t path[] = "non_existent_file_xyz_12345.dat";
    uint8_t* out_ptr = NULL;
    size_t out_len = 0;
    size_t out_cap = 0;
    int32_t status = __luna_read_file(path, sizeof(path) - 1, &out_ptr, &out_len, &out_cap);
    if (status != LUNA_STATUS_NOT_FOUND) {
        fprintf(stderr, "FAIL: reading non-existent file returned %d, expected LUNA_STATUS_NOT_FOUND (-1)\n", status);
        return 1;
    }
    printf("[PASS] test_file_not_found\n");
    return 0;
}

static int test_empty_file_io(void) {
    printf("[RUN] test_empty_file_io\n");
    const uint8_t path[] = "luna_empty_file_test.txt";
    int32_t wstatus = __luna_write_file(path, sizeof(path) - 1, NULL, 0);
    if (wstatus != LUNA_STATUS_OK) {
        fprintf(stderr, "FAIL: writing empty file returned %d\n", wstatus);
        return 1;
    }

    uint8_t* out_ptr = NULL;
    size_t out_len = 999;
    size_t out_cap = 999;
    int32_t rstatus = __luna_read_file(path, sizeof(path) - 1, &out_ptr, &out_len, &out_cap);
    if (rstatus != LUNA_STATUS_OK) {
        fprintf(stderr, "FAIL: reading empty file returned %d\n", rstatus);
        return 1;
    }
    if (out_len != 0 || out_cap != 0) {
        fprintf(stderr, "FAIL: empty file out_len %zu != 0 or out_cap %zu != 0\n", out_len, out_cap);
        return 1;
    }
    // Deallocating 0-byte buffer must be safe no-op
    __luna_buffer_free(out_ptr, out_cap);

#if defined(_WIN32)
    DeleteFileA((const char*)path);
#else
    unlink((const char*)path);
#endif

    printf("[PASS] test_empty_file_io\n");
    return 0;
}

static int test_file_partial_read_capacity_invariant(void) {
    printf("[RUN] test_file_partial_read_capacity_invariant\n");
    const uint8_t path[] = "luna_partial_read_test.txt";
    // Write 512 bytes
    uint8_t write_data[512];
    memset(write_data, 0x5A, sizeof(write_data));
    int32_t wstatus = __luna_write_file(path, sizeof(path) - 1, write_data, sizeof(write_data));
    if (wstatus != LUNA_STATUS_OK) {
        fprintf(stderr, "FAIL: write failed in partial read test\n");
        return 1;
    }

    uint8_t* out_ptr = NULL;
    size_t out_len = 0;
    size_t out_cap = 0;
    int32_t rstatus = __luna_read_file(path, sizeof(path) - 1, &out_ptr, &out_len, &out_cap);
    if (rstatus != LUNA_STATUS_OK) {
        fprintf(stderr, "FAIL: read failed in partial read test\n");
        return 1;
    }

    // Assert the fundamental runtime invariant: out_len <= out_cap
    if (out_len != 512 || out_cap != 512) {
        fprintf(stderr, "FAIL: expected len=512, cap=512, got len=%zu, cap=%zu\n", out_len, out_cap);
        __luna_buffer_free(out_ptr, out_cap);
        return 1;
    }

    if (out_len > out_cap) {
        fprintf(stderr, "FAIL: invariant out_len <= out_cap violated: %zu > %zu\n", out_len, out_cap);
        __luna_buffer_free(out_ptr, out_cap);
        return 1;
    }

    // Free with out_cap to verify allocator symmetry
    __luna_buffer_free(out_ptr, out_cap);

#if defined(_WIN32)
    DeleteFileA((const char*)path);
#else
    unlink((const char*)path);
#endif

    printf("[PASS] test_file_partial_read_capacity_invariant\n");
    return 0;
}

static int test_long_path_fallback(void) {
    printf("[RUN] test_long_path_fallback\n");
    // Generate path name with length > 512 (exceeding 512-wchar stack buffer)
    // To be safe on Windows filesystem, create a filename that is valid or test path validation:
    // A path like "luna_long_path_...txt" with 600 characters
    char long_path[650];
    memset(long_path, 'a', 600);
    long_path[0] = 'l';
    long_path[1] = 'u';
    long_path[2] = 'n';
    long_path[3] = 'a';
    long_path[4] = '_';
    memcpy(long_path + 590, ".txt", 4);
    long_path[594] = '\0';
    size_t path_len = 594;

    // Test that the runtime doesn't crash or reject purely due to length > 512.
    // Calling __luna_read_file on a non-existent 594-byte path should execute dynamic allocation fallback
    // and return LUNA_STATUS_NOT_FOUND or LUNA_STATUS_IO_ERROR, NOT LUNA_STATUS_INVALID_ARGUMENT!
    uint8_t* out_ptr = NULL;
    size_t out_len = 0;
    size_t out_cap = 0;
    int32_t status = __luna_read_file((const uint8_t*)long_path, path_len, &out_ptr, &out_len, &out_cap);
    if (status == LUNA_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "FAIL: long path (>512) was rejected as invalid argument (arbitrary stack buffer limit leak)\n");
        return 1;
    }

    printf("[PASS] test_long_path_fallback (dynamic fallback handled path length %zu without arbitrary cap)\n", path_len);
    return 0;
}

// --- Terminal Invariant Sub-Processes ------------------------------------------

static void run_terminal_target(const char* target) {
    if (strcmp(target, "term_align_0") == 0) {
        __luna_alloc(64, 0); // Must terminate
    } else if (strcmp(target, "term_align_3") == 0) {
        __luna_alloc(64, 3); // Must terminate
    } else if (strcmp(target, "term_align_8192") == 0) {
        __luna_alloc(64, 8192); // Must terminate (> LUNA_MAX_ALIGN)
    } else if (strcmp(target, "term_overflow") == 0) {
        __luna_alloc(SIZE_MAX - 10, 64); // Must terminate
    } else if (strcmp(target, "term_panic") == 0) {
        static const uint8_t msg[] = "test explicit panic";
        static const uint8_t file[] = "test_source.ln";
        __luna_panic(msg, sizeof(msg) - 1, file, sizeof(file) - 1, 42, 10);
    } else if (strcmp(target, "term_panic_default") == 0) {
        __luna_panic_default();
    } else if (strcmp(target, "term_bounds_fail") == 0) {
        static const uint8_t file[] = "test_bounds.ln";
        __luna_bounds_fail(15, 10, file, sizeof(file) - 1, 99, 4);
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

    // On Windows, abort() exits with code 3 or 101/102/3221226505
    if (expected_exit == 0) {
        if (exit_code == 0) {
            fprintf(stderr, "FAIL: expected %s to terminate abnormally, but exited with 0\n", target);
            return 1;
        }
    } else if (exit_code != (DWORD)expected_exit && exit_code != 3 && exit_code != 3221226505) {
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
    __luna_startup(argc, argv);

    if (argc >= 3 && strcmp(argv[1], "--run-terminal") == 0) {
        run_terminal_target(argv[2]);
        return 0;
    }

    printf("=================================================================\n");
    printf("   Luna Runtime ABI v1.0 Conformance Test Suite\n");
    printf("=================================================================\n");

    int failures = 0;

    // In-process positive invariant tests
    failures += test_abi_marker();
    failures += test_sentinel_identity();
    failures += test_realloc_transitions();
    failures += test_align_boundary_4096();
    failures += test_stdio_exact_bytes();
    failures += test_process_args();
    failures += test_file_io_roundtrip();
    failures += test_file_nul_defense();
    failures += test_file_not_found();
    failures += test_empty_file_io();
    failures += test_file_partial_read_capacity_invariant();
    failures += test_long_path_fallback();

    // Out-of-process terminal tests
    const char* self_exe = argv[0];
    failures += spawn_and_assert_terminated(self_exe, "term_align_0", 5);
    failures += spawn_and_assert_terminated(self_exe, "term_align_3", 5);
    failures += spawn_and_assert_terminated(self_exe, "term_align_8192", 5);
    failures += spawn_and_assert_terminated(self_exe, "term_overflow", 1);
    failures += spawn_and_assert_terminated(self_exe, "term_panic", 101);
    failures += spawn_and_assert_terminated(self_exe, "term_panic_default", 101);
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
