cmake_minimum_required(VERSION 3.20)

project(mellis-runtime VERSION 1.0.0 LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

if(MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

# Runtime ABI include directory (shared with compiler)
set(MELLIS_RUNTIME_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/include" CACHE PATH "Mellis runtime include directory")

include_directories(${MELLIS_RUNTIME_INCLUDE_DIR})

# ── Platform detection ────────────────────────────────────────────────────────
if(WIN32)
    set(MELLIS_PLATFORM "windows")
elseif(UNIX AND APPLE)
    set(MELLIS_PLATFORM "macos")
elseif(UNIX)
    set(MELLIS_PLATFORM "linux")
else()
    set(MELLIS_PLATFORM "freestanding")
endif()

message(STATUS "Mellis Runtime: building for platform '${MELLIS_PLATFORM}'")

# ── Source files ─────────────────────────────────────────────────────────────
set(RUNTIME_CORE_SRC
    src/memory/memory.c
    src/panic/panic.c
    src/io/io.c
    src/startup/startup.c
)

set(RUNTIME_PLATFORM_SRC
    src/platform/${MELLIS_PLATFORM}/threading.c
    src/platform/${MELLIS_PLATFORM}/sync.c
)

# ── Static library target ─────────────────────────────────────────────────────
add_library(mellis-runtime STATIC
    ${RUNTIME_CORE_SRC}
    ${RUNTIME_PLATFORM_SRC}
)

target_include_directories(mellis-runtime PUBLIC
    ${MELLIS_RUNTIME_INCLUDE_DIR}
)

# Link pthreads on POSIX
if(UNIX)
    target_link_libraries(mellis-runtime PRIVATE pthread)
endif()

# ── Install ───────────────────────────────────────────────────────────────────
install(TARGETS mellis-runtime
    ARCHIVE DESTINATION runtime/hosted/${MELLIS_PLATFORM}
)

install(DIRECTORY include/
    DESTINATION include
)
