cmake_minimum_required(VERSION 3.20)

project(luna-runtime VERSION 1.0.0 LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

if(MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

# Runtime ABI include directory (shared with compiler)
set(LUNA_RUNTIME_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/include" CACHE PATH "Luna runtime include directory")

include_directories(${LUNA_RUNTIME_INCLUDE_DIR})

# ── Platform detection ────────────────────────────────────────────────────────
if(WIN32)
    set(LUNA_PLATFORM "windows")
elseif(UNIX AND APPLE)
    set(LUNA_PLATFORM "macos")
elseif(UNIX)
    set(LUNA_PLATFORM "linux")
else()
    set(LUNA_PLATFORM "freestanding")
endif()

message(STATUS "Luna Runtime: building for platform '${LUNA_PLATFORM}'")

# ── Source files ─────────────────────────────────────────────────────────────
set(RUNTIME_CORE_SRC
    src/memory/memory.c
    src/panic/panic.c
    src/io/io.c
    src/startup/startup.c
)

set(RUNTIME_PLATFORM_SRC
    src/platform/${LUNA_PLATFORM}/threading.c
    src/platform/${LUNA_PLATFORM}/sync.c
)

# ── Static library target ─────────────────────────────────────────────────────
add_library(luna-runtime STATIC
    ${RUNTIME_CORE_SRC}
    ${RUNTIME_PLATFORM_SRC}
)

target_include_directories(luna-runtime PUBLIC
    ${LUNA_RUNTIME_INCLUDE_DIR}
)

# Link pthreads on POSIX
if(UNIX)
    target_link_libraries(luna-runtime PRIVATE pthread)
endif()

# ── Install ───────────────────────────────────────────────────────────────────
install(TARGETS luna-runtime
    ARCHIVE DESTINATION runtime/hosted/${LUNA_PLATFORM}
)

install(DIRECTORY include/
    DESTINATION include
)

# ── Conformance Tests ─────────────────────────────────────────────────────────
enable_testing()
add_executable(abi_tests tests/abi_tests.c)
target_link_libraries(abi_tests PRIVATE luna-runtime)
target_include_directories(abi_tests PRIVATE ${LUNA_RUNTIME_INCLUDE_DIR})
add_test(NAME RuntimeAbiTests COMMAND abi_tests)
