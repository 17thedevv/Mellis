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
#include "mellis/AST/DeclNode.h"
#include <string>
#include <vector>
#include <unordered_map>
#include <cstdint>

namespace fl {

class MacroRegistry;
class TypeContext;
class MLibMetadataCache;

class ModuleLoader {
public:
    ModuleLoader(SymbolTable& symbolTable,
                 DiagnosticEngine& diag,
                 const std::string& mainFileDir,
                 MacroRegistry* macroRegistry = nullptr,
                 const std::vector<std::string>& extraLibraryPaths = {},
                 TypeContext* typeContext = nullptr,
                 MLibMetadataCache* metadataCache = nullptr);

    // Load an external module by resolving the longest valid path prefix.
    // - Returns cached VirtualScopeID immediately if loaded.
    // - Sets `matchedSegments` to how many elements in `path` were consumed as the module path.
    // - If not found, emits an error and returns kInvalidScopeID.
    ScopeID loadModule(const std::vector<std::string_view>& path, SourceLocation loc, size_t& matchedSegments);

    // Check if a module has already been loaded (cache hit).
    bool isLoaded(std::string_view moduleName) const;

    // Get the absolute paths of all loaded .mlib files
    std::vector<std::string> getLoadedPaths() const;

private:
    enum class ModuleState { NotLoaded, Loading, Loaded, Failed };
    struct ModuleRecord {
        ModuleState state = ModuleState::NotLoaded;
        ScopeID scopeID = 0; // kInvalidScopeID usually 0, but we assume 0 is invalid here
    };

    SymbolTable& symbolTable;
    DiagnosticEngine& diag;
    std::string mainFileDir;
    MacroRegistry* macroRegistry;
    std::vector<std::string> searchPaths;
    TypeContext* typeContext;
    MLibMetadataCache* mlibCache;
    std::unordered_map<std::string, ModuleRecord> moduleRecords_;
    std::vector<std::string> loadedMLibPaths_;
    std::vector<std::vector<char>> loadedStringTables_;

    // Search for "<moduleName>.ms" and "<moduleName>.mlib", auto-compile .ms, return path to .mlib
    std::string resolveModulePath(std::string_view moduleName, SourceLocation loc);

    // Read the file into memory and parse Header + Metadata sections only.
    // Registers all exported Functions, Types, and Traits into virtualScope.
    void parseMLibMetadata(const std::string& path,
                           ScopeID virtualScope,
                           const uint8_t hintUUID[16],
                           std::string_view moduleName);

    // Parse the raw string table bytes and return offset-indexed strings.
    // StringTable layout: null-terminated strings packed contiguously.
    std::vector<char> loadStringSection(const std::vector<uint8_t>& fileData,
                                        uint64_t sectionOffset,
                                        uint64_t sectionSize);

    struct RawTypeRef {
        mlib::TypeRefRecord record;
        std::vector<uint32_t> payload;
    };

    // Register metadata entries from a section into the virtual scope.
    void registerFunctions(const std::vector<uint8_t>& fileData,
                           uint64_t sectionOffset,
                           uint64_t sectionSize,
                           ScopeID virtualScope,
                           const std::vector<char>& strings,
                           const uint8_t moduleUUID[16],
                           const std::vector<const Type*>& parsedTypeRefs);

    void registerTypes(const std::vector<uint8_t>& fileData,
                       uint64_t sectionOffset,
                       uint64_t sectionSize,
                       ScopeID virtualScope,
                       const std::vector<char>& strings,
                       const uint8_t moduleUUID[16],
                       const std::vector<const Type*>& parsedTypeRefs,
                       std::vector<std::pair<SymbolID, uint32_t>>& pendingSignatures);

    void registerImpls(const std::vector<uint8_t>& fileData,
                        uint64_t sectionOffset, uint64_t sectionSize,
                        ScopeID virtualScope, const std::vector<char>& strings,
                        const uint8_t moduleUUID[16],
                        const std::vector<const Type*>& parsedTypeRefs);

    std::vector<const Type*> parseTypeRefs(const std::vector<uint8_t>& fileData,
                                           uint64_t sectionOffset, uint64_t sectionSize,
                                           const std::vector<char>& strings,
                                           ScopeID virtualScope);



    void loadMacroMetadata(const std::vector<uint8_t>& fileData,
                           uint64_t sectionOffset, uint64_t sectionSize,
                           const std::vector<char>& strings,
                           std::string_view moduleName);

    void loadGenericMetadata(const std::vector<uint8_t>& fileData,
                             uint64_t sectionOffset, uint64_t sectionSize,
                             const std::vector<char>& strings,
                             ScopeID virtualScope,
                             std::string_view moduleName,
                             const uint8_t moduleUUID[16]);

public:
    // Exposes parsed generic Impl blocks for MonomorphizationEngine.
    // The AST nodes are owned by ModuleLoader (injectedGenerics_).
    std::vector<std::pair<SymbolID, class ImplDeclNode*>> getInjectedGenericImpls() const {
        return injectedImpls_;
    }
    
    std::vector<std::unique_ptr<class DeclNode>> takeInjectedGenerics() {
        return std::move(injectedGenerics_);
    }
    std::vector<std::string> takeInjectedStrings() {
        return std::move(injectedStrings_);
    }
private:
    std::vector<std::unique_ptr<class DeclNode>> injectedGenerics_;
    std::vector<std::pair<SymbolID, class ImplDeclNode*>> injectedImpls_;
    std::vector<std::string> injectedStrings_;
    
    // Loaded TypeRefs for the current module being loaded
    struct LoadedTypeRef {
        mlib::TypeRefRecord record;
        std::vector<uint32_t> payload;
    };
    std::vector<LoadedTypeRef> loadedTypeRefs_;
    std::vector<SymbolID> loadedTypes_;
    std::vector<SymbolID> loadedTraits_;
    
    const Type* reconstructType(uint32_t typeRefID, const std::vector<char>& strings, ScopeID virtualScope, TypeContext* typeCtx);
};
} // namespace fl

#endif // MELLIS_MLIB_MODULELOADER_H
