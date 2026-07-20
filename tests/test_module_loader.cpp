// =============================================================================
// tests/test_module_loader.cpp
//
// Verify ModuleLoader + ExternalSymbol integration.
// Uses an in-memory "fake" .mlib written with Phase M1-M5 builders.
// =============================================================================

#include "mellis/MLib/ModuleLoader.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/StringTableBuilder.h"
#include "mellis/MLib/ManifestBuilder.h"
#include "mellis/MLib/MetadataBuilder.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/Core/SourceLocation.h"

#include <iostream>
#include <cassert>
#include <fstream>
#include <vector>
#include <cstring>
#include <filesystem>

using namespace fl;
using namespace fl::mlib;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers: write a minimal valid .mlib file to disk for testing
// ─────────────────────────────────────────────────────────────────────────────

static void writeMiniMlib(const std::string& path) {
    // Build string table with our exported symbol names
    StringTableBuilder strings;
    uint32_t sidAdd    = strings.addString("add");
    uint32_t sidSub    = strings.addString("sub");
    uint32_t sidVec    = strings.addString("Vec");
    uint32_t sidOrd    = strings.addString("Ord");
    uint32_t sidModName= strings.addString("testmod");
    (void)sidModName;

    // Serialize string table
    BinaryWriter strWriter;
    strings.serialize(strWriter);
    auto strBytes = strWriter.takeBuffer();

    // Build ExportTable (Function entries: add, sub)
    BinaryWriter exportWriter;
    exportWriter.writeU32(1); // version
    exportWriter.writeU32(2); // count: add, sub
    {
        FunctionEntry fe;
        fe.nameStringID    = sidAdd;
        fe.namespaceID     = 0;
        fe.signatureTypeID = 0;
        exportWriter.writeStruct(fe);
    }
    {
        FunctionEntry fe;
        fe.nameStringID    = sidSub;
        fe.namespaceID     = 0;
        fe.signatureTypeID = 0;
        exportWriter.writeStruct(fe);
    }
    auto exportBytes = exportWriter.takeBuffer();

    // Build TypeMetadata (Vec)
    BinaryWriter typeWriter;
    typeWriter.writeU32(1); // version
    typeWriter.writeU32(1); // count
    {
        TypeEntry te;
        te.nameStringID  = sidVec;
        te.namespaceID   = 0;
        te.size          = 24;
        te.alignment     = 8;
        typeWriter.writeStruct(te);
    }
    auto typeBytes = typeWriter.takeBuffer();

    // Build TraitMetadata (Ord)
    BinaryWriter traitWriter;
    traitWriter.writeU32(1); // version
    traitWriter.writeU32(1); // count
    {
        TraitEntry tre;
        tre.nameStringID = sidOrd;
        tre.namespaceID  = 0;
        traitWriter.writeStruct(tre);
    }
    auto traitBytes = traitWriter.takeBuffer();

    // ── Assemble the .mlib binary ─────────────────────────────────────────────
    // Layout:
    //   [MLibHeader] [SectionTable: 4 entries] [String Data] [Export Data]
    //                                           [Type Data]   [Trait Data]

    const uint32_t NUM_SECTIONS = 4;
    const uint64_t headerSize   = sizeof(MLibHeader);
    const uint64_t tableSize    = NUM_SECTIONS * sizeof(SectionEntry);
    const uint64_t strOffset    = headerSize + tableSize;
    const uint64_t exportOffset = strOffset + strBytes.size();
    const uint64_t typeOffset   = exportOffset + exportBytes.size();
    const uint64_t traitOffset  = typeOffset + typeBytes.size();

    MLibHeader hdr{};
    std::memcpy(hdr.magic, MLIB_MAGIC, 4);
    hdr.formatVersion   = 1;
    hdr.compilerVersion = 1;
    // UUID: deterministic test UUID
    for (int i = 0; i < 16; ++i) hdr.moduleUUID[i] = static_cast<uint8_t>(i + 1);
    hdr.sectionCount       = NUM_SECTIONS;
    hdr.sectionTableOffset = headerSize;

    auto makeSection = [](uint32_t id, SectionType type, uint64_t off, uint64_t sz) {
        SectionEntry e{};
        e.sectionID   = id;
        e.sectionType = static_cast<uint32_t>(type);
        e.offset      = off;
        e.size        = sz;
        e.version     = 1;
        return e;
    };

    SectionEntry sections[NUM_SECTIONS] = {
        makeSection(1, SectionType::StringTable,   strOffset,    strBytes.size()),
        makeSection(2, SectionType::ExportTable,   exportOffset, exportBytes.size()),
        makeSection(3, SectionType::TypeMetadata,  typeOffset,   typeBytes.size()),
        makeSection(4, SectionType::TraitMetadata, traitOffset,  traitBytes.size()),
    };

    std::ofstream out(path, std::ios::binary);
    out.write(reinterpret_cast<const char*>(&hdr), sizeof(hdr));
    out.write(reinterpret_cast<const char*>(sections), sizeof(sections));
    out.write(reinterpret_cast<const char*>(strBytes.data()), strBytes.size());
    out.write(reinterpret_cast<const char*>(exportBytes.data()), exportBytes.size());
    out.write(reinterpret_cast<const char*>(typeBytes.data()), typeBytes.size());
    out.write(reinterpret_cast<const char*>(traitBytes.data()), traitBytes.size());
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

void testModuleLoaderBasic() {
    // Write a mini .mlib file into a temp directory
    std::string tmpDir = "build/test_mlib_tmp";
    std::filesystem::create_directories(tmpDir);
    std::string mlibPath = tmpDir + "/testmod.mlib";
    writeMiniMlib(mlibPath);

    DiagnosticEngine diag;
    SymbolTable symbolTable;

    ModuleLoader loader(symbolTable, diag, {tmpDir});

    // Test: module not yet loaded
    assert(!loader.isLoaded("testmod"));

    // Load the module
    ScopeID scope = loader.loadModule("testmod", SourceLocation::invalid());
    assert(scope != kInvalidScopeID);
    assert(loader.isLoaded("testmod"));
    assert(diag.errorCount() == 0);

    // Test: functions registered
    auto addSym = symbolTable.lookupInScope("add", scope);
    assert(addSym.has_value());
    assert(symbolTable.getSymbol(*addSym).kind == SymbolKind::Function);
    assert(symbolTable.getSymbol(*addSym).isExternal == true);

    auto subSym = symbolTable.lookupInScope("sub", scope);
    assert(subSym.has_value());
    assert(symbolTable.getSymbol(*subSym).kind == SymbolKind::Function);

    // Test: types registered
    auto vecSym = symbolTable.lookupInScope("Vec", scope);
    assert(vecSym.has_value());
    assert(symbolTable.getSymbol(*vecSym).kind == SymbolKind::Struct);
    assert(symbolTable.getSymbol(*vecSym).isExternal == true);

    // Test: traits registered
    auto ordSym = symbolTable.lookupInScope("Ord", scope);
    assert(ordSym.has_value());
    assert(symbolTable.getSymbol(*ordSym).kind == SymbolKind::Trait);

    // Test: UUID stored correctly
    const Symbol& addSymbol = symbolTable.getSymbol(*addSym);
    assert(addSymbol.externalModuleID[0] == 1);
    assert(addSymbol.externalModuleID[15] == 16);
}

void testModuleLoaderCacheHit() {
    std::string tmpDir = "build/test_mlib_tmp";

    DiagnosticEngine diag;
    SymbolTable symbolTable;
    ModuleLoader loader(symbolTable, diag, {tmpDir});

    ScopeID scope1 = loader.loadModule("testmod", SourceLocation::invalid());
    ScopeID scope2 = loader.loadModule("testmod", SourceLocation::invalid()); // cache hit

    assert(scope1 == scope2);
    assert(diag.errorCount() == 0);
}

void testModuleLoaderMissingModule() {
    DiagnosticEngine diag;
    SymbolTable symbolTable;
    ModuleLoader loader(symbolTable, diag, {"/nonexistent/path"});

    ScopeID scope = loader.loadModule("ghost", SourceLocation::invalid());
    assert(scope == kInvalidScopeID);
    assert(diag.errorCount() > 0); // Error: "Cannot find module 'ghost'"
}

int main() {
    testModuleLoaderBasic();
    std::cout << "testModuleLoaderBasic passed\n";

    testModuleLoaderCacheHit();
    std::cout << "testModuleLoaderCacheHit passed\n";

    testModuleLoaderMissingModule();
    std::cout << "testModuleLoaderMissingModule passed\n";

    std::cout << "All ModuleLoader tests passed!\n";
    return 0;
}
