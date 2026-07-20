# Implementation Plan: Phase M1 - MLib Binary Infrastructure

This plan outlines the architecture and tasks for implementing Phase M1 of the `.mlib` (Mellis Library Binary) ecosystem.

## Goal
Establish the foundational binary infrastructure for `.mlib`, enabling the compiler to read and write binary files without any LLVM dependencies, using MVIR as the intermediate representation.

## Open Questions
> [!IMPORTANT]
> 1. **TargetTriple Representation:** Since `.mlib` must not depend on LLVM, should we define a minimal internal `TargetTriple` struct (e.g., `enum Architecture`, `enum OS`, `enum Environment`), or use a raw `std::string`? I propose using a fixed-size `char target[64]` or internal enums to avoid string allocation during binary reads.
> 2. **UUID Representation:** For `moduleUUID`, is a standard `uint8_t uuid[16]` sufficient, or do you have a specific UUID library you prefer?
> 3. **Memory Mapping (mmap):** The roadmap schedules "mmap optimization" for Phase M5, but the architecture explicitly states "mmap()" for reading. For Phase M1, I propose designing `BinaryReader` to accept a raw memory buffer (`const uint8_t* data, size_t size`). This allows us to use `std::vector` for reading files now, and seamlessly switch to OS-level `mmap` in Phase M5 without changing the reader API. Does this sound good?

## Proposed Changes

### 1. File Structure and Integration
Create a new `MLib` subsystem in the codebase.
- **[NEW]** `include/mellis/MLib/MLibFormat.h`
- **[NEW]** `include/mellis/MLib/BinaryWriter.h`
- **[NEW]** `include/mellis/MLib/BinaryReader.h`
- **[NEW]** `include/mellis/MLib/Serializer.h`
- **[NEW]** `include/mellis/MLib/Deserializer.h`
- **[NEW]** `src/MLib/BinaryWriter.cpp`
- **[NEW]** `src/MLib/BinaryReader.cpp`
- **[NEW]** `src/MLib/Serializer.cpp`
- **[NEW]** `src/MLib/Deserializer.cpp`
- **[MODIFY]** `CMakeLists.txt` (Add `MLIB_SRC` variable and create a new test target `test_mlib`)

### 2. Format Definitions (`MLibFormat.h`)
Define the core data structures that map directly to the binary format.
- `MLibHeader` struct (magic bytes "MLIB", version, target, hash, sectionCount, timestamp, uuid).
- `SectionType` enum (ExportTable, TypeMetadata, TraitMetadata, GenericMVIR, etc.).
- `SectionEntry` struct (type, offset, size, hash).

### 3. Binary Reader & Writer
Low-level abstractions for reading/writing primitive types (integers, floats, raw bytes) to/from binary.
- `BinaryWriter`: Wraps a `std::vector<uint8_t>` or `std::ofstream`. Ensures correct endianness (Little Endian by default for consistency).
- `BinaryReader`: Wraps a `const uint8_t*` buffer. Performs bounds checking and reads primitives.

### 4. Serializer & Deserializer Framework
High-level abstractions built on top of `BinaryReader/Writer` to handle Mellis-specific data types (e.g., strings, vectors, UUIDs, target triples).
- `Serializer`: Exposes methods like `writeString()`, `writeVector()`, etc.
- `Deserializer`: Exposes methods like `readString()`, `readVector()`, etc.

## Verification Plan

### Automated Tests
- **[NEW]** `tests/test_mlib.cpp`: 
  - Test writing a valid `MLibHeader` and reading it back.
  - Test writing and reading multiple `SectionEntry` items.
  - Test the `Serializer` and `Deserializer` framework for various primitive and standard types.
  - Test binary integrity (ensure magic bytes are exactly "MLIB").

### Execution
Run `cmake --build build --target test_mlib` and ensure all tests pass and no memory leaks are detected.
