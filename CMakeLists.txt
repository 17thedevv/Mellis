cmake_minimum_required(VERSION 3.20)
project(luna-toolchain VERSION 1.0.0 LANGUAGES CXX C)

# ==========================================
# Root build system — Luna Toolchain
#
# Subprojects:
#   runtime/    → luna-runtime.lib / .a
# ==========================================

add_subdirectory(runtime)

# ── Output layout ─────────────────────────────────────────────────────────────
# Build artifacts go to build/host/{platform}/{config}/
# This keeps source tree clean.

if(MSVC)
    set(LUNA_HOST_TRIPLE "x86_64-pc-windows-msvc")
elseif(UNIX AND APPLE)
    set(LUNA_HOST_TRIPLE "x86_64-apple-macosx")
else()
    set(LUNA_HOST_TRIPLE "x86_64-unknown-linux-gnu")
endif()

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/host/${LUNA_HOST_TRIPLE}/$<CONFIG>")
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/host/${LUNA_HOST_TRIPLE}/$<CONFIG>")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/host/${LUNA_HOST_TRIPLE}/$<CONFIG>")

message(STATUS "Luna toolchain output: ${CMAKE_BINARY_DIR}/host/${LUNA_HOST_TRIPLE}/")

enable_testing()
