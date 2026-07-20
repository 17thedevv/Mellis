#ifndef MELLIS_MLIB_MODULELOADER_H
#define MELLIS_MLIB_MODULELOADER_H

// =============================================================================
// mellis/MLib/ModuleLoader.h
//
// ModuleLoader — the "librarian" that bridges .mlib binaries into the
// compiler's live SymbolTable.
//
// Responsibilities (single-pass, lazy):
//   1. FIND:     Locate `<name>.mlib` in the library search paths.
//   2. LOAD:     Read only Header + Metadata sections (strings, functions,
//                types, traits). Does NOT load GenericMVIR or ObjectCode.
//   3. REGISTER: Create a VirtualScope in SymbolTable and populate it with
//                ExternalSymbols backed by MLib SymbolIDs.
//   4. CACHE:    Return the same ScopeID on repeated calls for the same module.
//
// Phase boundary: ModuleLoader is owned by ImportResolver (FrontEnd).
// It must not call into TypeChecker, Optimizer, or Backend.
// =============================================================================

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryReader.h"
#include "mellis/MLib/StringTableBuilder.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/Support/Diagnostic.h"
#include <string>
#include <vector>
#include <unordered_map>
#include <cstdint>

namespace fl {

class ModuleLoader {
public:
    ModuleLoader(SymbolTable& symbolTable,
                 DiagnosticEngine& diag,
                 const std::vector<std::string>& extraLibraryPaths = {});

    // Load an external module by name (e.g. "std").
    // - If already loaded, returns cached VirtualScopeID immediately (O(1)).
    // - If not found in any library path, emits an error and returns kInvalidScopeID.
    ScopeID loadModule(std::string_view moduleName, SourceLocation loc);

    // Check if a module has already been loaded (cache hit).
    bool isLoaded(std::string_view moduleName) const;

    // Get the absolute paths of all loaded .mlib files
    std::vector<std::string> getLoadedPaths() const;

private:
    SymbolTable& symbolTable;
    DiagnosticEngine& diag;
    std::vector<std::string> searchPaths;
    std::unordered_map<std::string, ScopeID> loadedModules;
    std::vector<std::string> loadedMLibPaths_;

    // Search libraryPaths for "<moduleName>.mlib". Returns "" if not found.
    std::string findMLibFile(std::string_view moduleName) const;

    // Read the file into memory and parse Header + Metadata sections only.
    // Registers all exported Functions, Types, and Traits into virtualScope.
    void parseMLibMetadata(const std::string& path,
                           ScopeID virtualScope,
                           const uint8_t moduleUUID[16]);

    // Parse the raw string table bytes and return offset-indexed strings.
    // StringTable layout: null-terminated strings packed contiguously.
    std::vector<char> loadStringSection(const std::vector<uint8_t>& fileData,
                                        uint64_t sectionOffset,
                                        uint64_t sectionSize);

    // Register metadata entries from a section into the virtual scope.
    void registerFunctions(const std::vector<uint8_t>& fileData,
                           uint64_t sectionOffset,
                           uint64_t sectionSize,
                           ScopeID virtualScope,
                           const std::vector<char>& strings,
                           const uint8_t moduleUUID[16]);

    void registerTypes(const std::vector<uint8_t>& fileData,
                       uint64_t sectionOffset,
                       uint64_t sectionSize,
                       ScopeID virtualScope,
                       const std::vector<char>& strings,
                       const uint8_t moduleUUID[16]);

    void registerTraits(const std::vector<uint8_t>& fileData,
                        uint64_t sectionOffset,
                        uint64_t sectionSize,
                        ScopeID virtualScope,
                        const std::vector<char>& strings,
                        const uint8_t moduleUUID[16]);

};

} // namespace fl

#endif // MELLIS_MLIB_MODULELOADER_H
