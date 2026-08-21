cmake_minimum_required(VERSION 3.20)
project(mellis-toolchain VERSION 1.0.0 LANGUAGES CXX C)

# ==========================================
# Root build system — Mellis Toolchain
#
# Subprojects:
#   compiler/   → mellis.exe
#   runtime/    → mellis-runtime.lib / .a
# ==========================================

add_subdirectory(compiler)
add_subdirectory(runtime)

# ── Output layout ─────────────────────────────────────────────────────────────
# Build artifacts go to build/host/{platform}/{config}/
# This keeps source tree clean.

if(MSVC)
    set(MELLIS_HOST_TRIPLE "x86_64-pc-windows-msvc")
elseif(UNIX AND APPLE)
    set(MELLIS_HOST_TRIPLE "x86_64-apple-macosx")
else()
    set(MELLIS_HOST_TRIPLE "x86_64-unknown-linux-gnu")
endif()

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/host/${MELLIS_HOST_TRIPLE}/$<CONFIG>")
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/host/${MELLIS_HOST_TRIPLE}/$<CONFIG>")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/host/${MELLIS_HOST_TRIPLE}/$<CONFIG>")

message(STATUS "Mellis toolchain output: ${CMAKE_BINARY_DIR}/host/${MELLIS_HOST_TRIPLE}/")

enable_testing()
