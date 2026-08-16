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
    Manifest = 10,
    ImplTable = 11,
    MacroMetadata = 12,
    GenericMetadata = 13,
    Custom = 0xFFFFFFFF
};

enum class GenericKind : uint8_t {
    Function = 0,
    Struct = 1,
    Enum = 2,
    Impl = 3,
    Trait = 4,
    TypeAlias = 5
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
    uint16_t mvirVersion;      // 1 for v1.0
    char targetTriple[64];     // Target triple string, null-terminated
    uint8_t moduleUUID[16];    // 128-bit UUID for the module
    uint64_t moduleHash;       // Hash of the module (or padding if unused)
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

// ---------------------------------------------------------
// Phase M2: Manifest & Dependency
// ---------------------------------------------------------

struct ManifestHeader {
    uint32_t nameStringID;
    uint32_t authorStringID;
    uint32_t versionStringID;
    uint32_t licenseStringID;
    uint32_t featureCount;
    uint32_t dependencyCount;
};

enum class ImportMode : uint8_t {
    Private = 0,
    Public = 1,
    Optional = 2
};

struct DependencyEntry {
    uint8_t moduleUUID[16];
    uint32_t versionStringID;
    uint64_t moduleHash;
    ImportMode importMode;
    uint32_t featureCount; // Followed by featureCount * StringID
};

// ---------------------------------------------------------
// Phase M2: Metadata Arena Tables
// ---------------------------------------------------------

struct NamespaceEntry {
    uint32_t nameStringID;
    uint32_t parentNamespaceID; // 0xFFFFFFFF if root
};

struct TypeEntry {
    uint32_t nameStringID;
    uint32_t namespaceID;
    uint64_t size;
    uint64_t alignment;
    uint8_t  visibility;
    uint32_t moduleID;
};

struct TraitEntry {
    uint32_t nameStringID;
    uint32_t namespaceID;
    uint8_t  visibility;
    uint32_t moduleID;
};

struct FunctionEntry {
    uint32_t nameStringID;
    uint32_t namespaceID;
    uint32_t signatureTypeID;  // index into the SignatureTable
    uint8_t  isVariadic;
    uint8_t  paramCount;
    uint16_t flags;            // reserved
    uint8_t  visibility;
    uint32_t moduleID;
};

// One parameter slot in a function signature.
// Stored as a flat list; FunctionEntry.paramCount says how many follow.
struct ExternalParamEntry {
    uint32_t nameStringID;     // Parameter name (for named-arg call sites)
    uint8_t  primitiveKind;    // If > 0: maps to BuiltinKind directly
    uint8_t  typeKind;         // 0=primitive, 1=struct, 2=unknown/opaque
    uint16_t reserved;
    uint32_t structSymbolID;   // Valid only when typeKind == 1
};

// Return type encoding: same layout as ExternalParamEntry, no name.
struct ExternalReturnEntry {
    uint8_t  primitiveKind;
    uint8_t  typeKind;
    uint16_t reserved;
    uint32_t structSymbolID;
};

struct ImplEntry {
    uint32_t traitID;
    uint32_t targetTypeID;
};

// ---------------------------------------------------------
// Phase M3: Generic MVIR Packaging
// ---------------------------------------------------------

struct GenericFunctionIndex {
    uint32_t functionID;
    uint32_t compressedSize;
    uint32_t uncompressedSize;
    uint64_t offset;           // Absolute offset in the Generic MVIR Section
    uint64_t mvirHash;
    uint64_t constraintHash;
    uint64_t signatureHash;
};

struct GenericPackageHeader {
    uint32_t functionID;
    uint16_t typeParamCount;
    uint16_t lifetimeParamCount;
    uint16_t constraintCount;
    uint16_t flags;
    uint32_t returnTypeID;
};

// ---------------------------------------------------------
// Phase M4: Object Code Section
// ---------------------------------------------------------

struct ObjectFunctionIndex {
    uint32_t functionID;
    uint32_t size;             // Byte count of the raw object blob
    uint64_t offset;           // Absolute byte offset within the Object Code Section
    uint64_t objectHash;       // XXHash64 of the raw object bytes (for incremental linking)
};

// ---------------------------------------------------------
// Phase M5: Debug Section
// ---------------------------------------------------------

// Line mapping entry: maps a MVIR instruction (by index) to a source location.
struct DebugLineEntry {
    uint32_t functionID;
    uint32_t instructionIndex; // Flat index within the function's basic blocks
    uint32_t fileStringID;     // StringID of the source file path
    uint32_t line;
    uint32_t column;
};

// ---------------------------------------------------------
// Phase M5: Function Dependency Graph
// ---------------------------------------------------------

// One edge in the function call graph: callerID calls calleeID.
// Stored as a flat list of directed edges. Compiler reconstructs the
// full graph and computes reverse-reachability for incremental builds.
struct DepEdge {
    uint32_t callerID;
    uint32_t calleeID;
};

// ---------------------------------------------------------
// Phase M5: Documentation Section
// ---------------------------------------------------------

// One doc-comment record: functionID / typeID + string offset.
// Used by IDE for hover/tooltip without loading MVIR at all.
struct DocEntry {
    uint32_t symbolID;         // FunctionID or TypeID
    uint32_t docStringID;      // StringID of the doc comment text
};

#pragma pack(pop)

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_MLIBFORMAT_H
