#ifndef MELLIS_MLIB_MLIBFORMAT_H
#define MELLIS_MLIB_MLIBFORMAT_H

#include <cstdint>
#include <cstring>
#include <cstddef>

namespace fl {
namespace mlib {

// Magic bytes for MLIB files
constexpr char MLIB_MAGIC[4] = {'M', 'L', 'I', 'B'};

// Section types defined in the v1.0 architecture
enum class SectionType : uint32_t {
    ExportTable = 1,
    TypeMetadata = 2,
    TraitMetadata = 3,
    StringTable = 4,
    DependencyTable = 5,
    FeatureTable = 6,
    GenericMVIR = 7,
    ObjectCode = 8,
    Debug = 9,
    Custom = 0xFFFFFFFF
};

enum class SectionCompression : uint8_t {
    None = 0,
    LZ4 = 1,
    ZSTD = 2
};

#pragma pack(push, 1)

// The main header structure for a .mlib binary
struct MLibHeader {
    char magic[4];             // "MLIB"
    uint16_t formatVersion;    // 1 for v1.0
    uint16_t compilerVersion;  // Specific compiler build version
    char targetTriple[64];     // Target triple string, null-terminated
    uint8_t moduleUUID[16];    // 128-bit UUID for the module
    uint64_t moduleHash;       // Hash of the module
    uint64_t timestamp;        // UNIX timestamp of build
    uint32_t flags;            // Flags (Debug, Optimized, Reflection, Compressed, Signed)
    uint32_t sectionCount;     // Number of sections
    uint64_t sectionTableOffset;// Offset to the section table from start of file
};

// Represents an entry in the Section Table
struct SectionEntry {
    uint32_t sectionID;           // Unique ID of the section within the module
    uint32_t sectionType;         // Maps to SectionType enum
    uint64_t offset;              // Absolute offset in the binary
    uint64_t size;                // Size of the section data in bytes
    uint16_t version;             // Version of the specific section data format
    SectionCompression compression;// Compression algorithm used
    uint8_t reserved[5];          // Padding for alignment
    uint64_t hash;                // XXHash64 of the section data
};

#pragma pack(pop)

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_MLIBFORMAT_H
