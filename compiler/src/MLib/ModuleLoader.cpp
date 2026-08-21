// =============================================================================
// mellis/MLib/ModuleLoader.cpp
// =============================================================================

#include "mellis/MLib/ModuleLoader.h"
#include "mellis/MLib/MLibFormat.h"
#include "mellis/Support/OSUtils.h"
#include "mellis/FrontEnd/Lexer.h"
#include "mellis/FrontEnd/Parser.h"
#include "mellis/MiddleEnd/Resolver.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include "mellis/Core/CompilerSession.h" // For child compilation
#include <iostream>
#include <fstream>
#include <cstring>
#include <stdexcept>
#include <filesystem>
#include <cstdlib>
#include <set>

namespace fl {

using namespace mlib;

// ─────────────────────────────────────────────────────────────────────────────
// Constructor
// ─────────────────────────────────────────────────────────────────────────────

ModuleLoader::ModuleLoader(SymbolTable& symbolTable,
                           DiagnosticEngine& diag,
                           const std::string& mainFileDir,
                           MacroRegistry* macroRegistry,
                           const std::vector<std::string>& extraLibraryPaths,
                           TypeContext* typeContext,
                           MLibMetadataCache* metadataCache)
    : symbolTable(symbolTable), diag(diag), mainFileDir(mainFileDir), macroRegistry(macroRegistry), typeContext(typeContext), mlibCache(metadataCache) {
    namespace fs = std::filesystem;

    // 1. Current Module Directory
    if (!mainFileDir.empty()) {
        searchPaths.push_back(mainFileDir);
    }

    // 2. Project Root (fallback to CWD)
    std::error_code ec;
    fs::path cwd = fs::current_path(ec);
    if (!ec) {
        searchPaths.push_back(cwd.string());
        searchPaths.push_back((cwd / "lib").string());
        searchPaths.push_back((cwd / "libs").string());
        if (!mainFileDir.empty()) {
            searchPaths.push_back((fs::path(mainFileDir) / "lib").string());
        }
    }

    // 3. Explicit -L paths
    for (const auto& path : extraLibraryPaths) {
        searchPaths.push_back(path);
    }

    // 4. Environment Variable
    if (const char* envPath = std::getenv("MELLIS_PATH")) {
        searchPaths.push_back(envPath);
    }

    // 5. Sysroot
    std::string exePath = OSUtils::getExecutablePath();
    std::string sysroot = OSUtils::getParentDirectory(exePath, 2); // go up from bin/
    searchPaths.push_back((fs::path(sysroot) / "lib").string());
    searchPaths.push_back((fs::path(OSUtils::getParentDirectory(exePath, 1)) / "lib").string());
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

bool ModuleLoader::isLoaded(std::string_view moduleName) const {
    auto it = moduleRecords_.find(std::string(moduleName));
    return it != moduleRecords_.end() && it->second.state == ModuleState::Loaded;
}

std::vector<std::string> ModuleLoader::getLoadedPaths() const {
    return loadedMLibPaths_;
}

ScopeID ModuleLoader::loadModule(const std::vector<std::string_view>& path, SourceLocation loc, size_t& matchedSegments) {
    if (path.empty()) return kInvalidScopeID;
    
    // We try to match the longest prefix.
    // e.g. path = ["graphics", "renderer", "ping"]
    // We try "graphics/renderer/ping", then "graphics/renderer", then "graphics"
    std::string moduleName;
    std::string mlibPath;
    
    for (size_t i = 1; i <= path.size(); ++i) {
        moduleName = "";
        for (size_t j = 0; j < i; ++j) {
            if (j > 0) moduleName += "/";
            moduleName += std::string(path[j]);
        }
        
        std::string resolved = resolveModulePath(moduleName, loc);
        if (!resolved.empty()) {
            mlibPath = resolved;
            matchedSegments = i;
            break;
        }
    }
    
    if (mlibPath.empty()) {
        moduleName = std::string(path[0]);
        for(size_t j = 1; j < path.size(); ++j) { moduleName += "/" + std::string(path[j]); }
        diag.error(loc, "Cannot find module matching path '" + moduleName + "' in library paths.");
        return kInvalidScopeID;
    }
    
    // Use the matched moduleName as key
    std::string key = moduleName;

    auto it = moduleRecords_.find(key);
    if (it != moduleRecords_.end()) {
        if (it->second.state == ModuleState::Loading) {
            diag.error(loc, "Circular import detected for module '" + key + "'.");
            return kInvalidScopeID;
        }
        if (it->second.state == ModuleState::Loaded) {
            return it->second.scopeID;
        }
        if (it->second.state == ModuleState::Failed) {
            return kInvalidScopeID;
        }
    }

    moduleRecords_[key] = {ModuleState::Loading, kInvalidScopeID};

    // Create the virtual scope for this module.
    // Use the last segment as the scope name
    std::string scopeName = std::string(path[matchedSegments - 1]);
    ScopeID virtualScope = symbolTable.createVirtualModuleScope(scopeName);
    
    // Parse only Header + Metadata — no MVIR, no ObjectCode.
    try {
        parseMLibMetadata(mlibPath, virtualScope, nullptr, moduleName);
        loadedMLibPaths_.push_back(mlibPath);
        moduleRecords_[key] = {ModuleState::Loaded, virtualScope};
    } catch (const std::exception& ex) {
        diag.error(loc, std::string("Failed to load module '") + key + "': " + ex.what());
        moduleRecords_[key] = {ModuleState::Failed, kInvalidScopeID};
        return kInvalidScopeID;
    }

    return virtualScope;
}

// ─────────────────────────────────────────────────────────────────────────────
// File Resolution & Internal Compilation
// ─────────────────────────────────────────────────────────────────────────────

std::string ModuleLoader::resolveModulePath(std::string_view moduleName, SourceLocation loc) {
    namespace fs = std::filesystem;
    std::string relPath = std::string(moduleName);
    std::string msName = relPath + ".ms";
    std::string mlibName = relPath + ".mlib";
    
    std::string foundMsPath;
    std::string foundMlibPath;
    
    for (const auto& dir : searchPaths) {
        fs::path msPath = fs::path(dir) / msName;
        fs::path mlibPath = fs::path(dir) / mlibName;
        
        std::cout << "[DEBUG] Checking " << msPath.string() << " and " << mlibPath.string() << "\n";
        
        bool hasMs = fs::exists(msPath);
        bool hasMlib = fs::exists(mlibPath);
        
        if (hasMs || hasMlib) {
            foundMsPath = hasMs ? msPath.string() : "";
            foundMlibPath = hasMlib ? mlibPath.string() : "";
            break; // Stop at highest priority tier
        }
        
        // Also check for directory modules: relPath/mod.ms and relPath/mod.mlib
        fs::path dirMsPath = fs::path(dir) / relPath / "mod.ms";
        fs::path dirMlibPath = fs::path(dir) / relPath / "mod.mlib";
        
        std::cout << "[DEBUG] Checking " << dirMsPath.string() << " and " << dirMlibPath.string() << "\n";
        
        bool hasDirMs = fs::exists(dirMsPath);
        bool hasDirMlib = fs::exists(dirMlibPath);
        
        if (hasDirMs || hasDirMlib) {
            foundMsPath = hasDirMs ? dirMsPath.string() : "";
            foundMlibPath = hasDirMlib ? dirMlibPath.string() : "";
            break; // Stop at highest priority tier
        }
    }
    
    if (foundMsPath.empty() && foundMlibPath.empty()) {
        return "";
    }
    
    bool needCompile = false;
    if (foundMlibPath.empty()) {
        needCompile = true;
        // Output .mlib next to .ms
        foundMlibPath = foundMsPath.substr(0, foundMsPath.find_last_of('.')) + ".mlib";
    } else if (!foundMsPath.empty()) {
        // Compare timestamps
        std::error_code ec1, ec2;
        auto msTime = fs::last_write_time(foundMsPath, ec1);
        auto mlibTime = fs::last_write_time(foundMlibPath, ec2);
        if (!ec1 && !ec2 && msTime > mlibTime) {
            needCompile = true;
        }
    }
    
    if (needCompile) {
        // Guard against infinite recursive compilation (e.g. self-importing packages)
        static thread_local std::set<std::string> compilingModules;
        std::string canonicalPath = fs::canonical(fs::path(foundMsPath)).string();
        if (compilingModules.count(canonicalPath)) {
            diag.error(loc, "Circular compilation detected for: " + foundMsPath);
            return "";
        }
        compilingModules.insert(canonicalPath);

        std::cout << "[ModuleLoader] Compiling dependency " << foundMsPath << "..." << std::endl;
        CompilerSession childSession;
        // Don't recurse extraLibraryPaths excessively, just pass them
        childSession.setLibraryPaths(searchPaths); 
        bool ok = childSession.compile(foundMsPath, false, 0, true);

        compilingModules.erase(canonicalPath);

        if (!ok) {
            diag.error(loc, "Failed to compile dependency: " + foundMsPath);
            return "";
        }
    }
    
    return foundMlibPath;
}


// ─────────────────────────────────────────────────────────────────────────────
// Binary Parsing — Header + Metadata Sections (Lazy Load)
// ─────────────────────────────────────────────────────────────────────────────

void ModuleLoader::parseMLibMetadata(const std::string& path,
                                     ScopeID virtualScope,
                                     const uint8_t hintUUID[16],
                                     std::string_view moduleName) {
    // Read the entire file into memory.
    // The file is small at this stage (only strings + metadata tables).
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    if (!file.is_open()) {
        throw std::runtime_error("Cannot open file: " + path);
    }

    std::streamsize fileSize = file.tellg();
    file.seekg(0, std::ios::beg);
    std::vector<uint8_t> fileData(static_cast<size_t>(fileSize));
    if (!file.read(reinterpret_cast<char*>(fileData.data()), fileSize)) {
        throw std::runtime_error("Failed to read file: " + path);
    }

    if (fileSize < static_cast<std::streamsize>(sizeof(MLibHeader))) {
        throw std::runtime_error("File too small to be a valid .mlib: " + path);
    }

    // Validate magic bytes.
    const MLibHeader* header = reinterpret_cast<const MLibHeader*>(fileData.data());
    if (std::memcmp(header->magic, MLIB_MAGIC, 4) != 0) {
        throw std::runtime_error("Invalid .mlib magic bytes in: " + path);
    }

    const uint8_t* moduleUUID = header->moduleUUID;

    // Scan the section table.
    uint64_t tableOffset = header->sectionTableOffset;
    uint32_t sectionCount = header->sectionCount;

    if (tableOffset + sectionCount * sizeof(SectionEntry) > static_cast<uint64_t>(fileSize)) {
        std::cout << "DEBUG ModuleLoader: tableOffset=" << tableOffset << " sectionCount=" << sectionCount << " fileSize=" << fileSize << "\n";
        throw std::runtime_error("Section table out of bounds in: " + path);
    }

    std::vector<char> strings;
    // Two-pass: first load strings, then load symbol metadata.
    // Pass 1: find and load the StringTable section.
    const SectionEntry* sections = reinterpret_cast<const SectionEntry*>(fileData.data() + tableOffset);
    for (uint32_t i = 0; i < sectionCount; ++i) {
        if (static_cast<SectionType>(sections[i].sectionType) == SectionType::StringTable) {
            strings = loadStringSection(fileData, sections[i].offset, sections[i].size);
            break;
        }
    }

    // Pass 2: register Types (Namespaces, Structs, Traits) into the virtual scope.
    std::vector<std::pair<SymbolID, uint32_t>> pendingSignatures;
    for (uint32_t i = 0; i < sectionCount; ++i) {
        if (static_cast<SectionType>(sections[i].sectionType) == SectionType::TypeMetadata) {
            registerTypes(fileData, sections[i].offset, sections[i].size,
                           virtualScope, strings, moduleUUID, {}, pendingSignatures);
            break;
        }
    }

    // Pass 2.5: load GenericMetadata (needs to be in virtualScope for TypeRefTable)
    for (uint32_t i = 0; i < sectionCount; ++i) {
        if (static_cast<SectionType>(sections[i].sectionType) == SectionType::GenericMetadata) {
            loadGenericMetadata(fileData, sections[i].offset, sections[i].size,
                                strings, virtualScope, moduleName, moduleUUID);
            break;
        }
    }

    // Pass 3: find and parse TypeRefTable (can now use virtualScope for Named types)
    std::vector<const Type*> parsedTypeRefs;
    for (uint32_t i = 0; i < sectionCount; ++i) {
        if (static_cast<SectionType>(sections[i].sectionType) == SectionType::TypeRefTable) {
            parsedTypeRefs = parseTypeRefs(fileData, sections[i].offset, sections[i].size, strings, virtualScope);
            break;
        }
    }

    // Process pending signatures now that we have parsedTypeRefs
    for (auto& p : pendingSignatures) {
        if (mlibCache && p.second < parsedTypeRefs.size()) {
            const Type* sigType = parsedTypeRefs[p.second];
            if (sigType) {
                mlibCache->registerType(p.first, sigType);
                std::cout << "[MLibLoader] Registered signature later symId=" << p.first << " type=" << sigType->toString() << "\n";
            }
        }
    }

    // Pass 4: register Functions, Impls, Macro, GenericMetadata
    for (uint32_t i = 0; i < sectionCount; ++i) {
        auto type = static_cast<SectionType>(sections[i].sectionType);
        switch (type) {
            case SectionType::ExportTable:
                registerFunctions(fileData, sections[i].offset, sections[i].size,
                                  virtualScope, strings, moduleUUID, parsedTypeRefs);
                break;
            case SectionType::ImplTable:
                registerImpls(fileData, sections[i].offset, sections[i].size,
                                virtualScope, strings, moduleUUID, parsedTypeRefs);
                break;
            case SectionType::MacroMetadata:
                loadMacroMetadata(fileData, sections[i].offset, sections[i].size,
                                  strings, moduleName);
                break;
            default:
                break;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// String Table
// ─────────────────────────────────────────────────────────────────────────────

std::vector<char> ModuleLoader::loadStringSection(const std::vector<uint8_t>& fileData,
                                                   uint64_t sectionOffset,
                                                   uint64_t sectionSize) {
    if (sectionOffset + sectionSize > fileData.size()) {
        throw std::runtime_error("StringTable section out of bounds");
    }
    const char* start = reinterpret_cast<const char*>(fileData.data() + sectionOffset);
    return std::vector<char>(start, start + sectionSize);
}

// Helper: resolve a StringID (offset into the string blob) to a string_view.
static std::string_view resolveString(const std::vector<char>& strings, uint32_t stringID) {
    if (stringID >= strings.size()) return "<invalid>";
    return std::string_view(strings.data() + stringID);
}

// ─────────────────────────────────────────────────────────────────────────────
// Registration Helpers
// ─────────────────────────────────────────────────────────────────────────────

void ModuleLoader::registerFunctions(const std::vector<uint8_t>& fileData,
                                     uint64_t sectionOffset,
                                     uint64_t sectionSize,
                                     ScopeID virtualScope,
                                     const std::vector<char>& strings,
                                     const uint8_t moduleUUID[16],
                                     const std::vector<const Type*>& parsedTypeRefs) {
    if (sectionSize == 0) return;

    BinaryReader reader(fileData.data() + sectionOffset, sectionSize);
    uint32_t version = reader.readU32();
    (void)version; // Forward-compat: ignore unknown fields

    std::cout << "[MLibLoader] Loading Functions...\n";
    uint32_t count = reader.readU32();
    std::cout << "[MLibLoader] Found " << count << " functions\n";
    for (uint32_t i = 0; i < count; ++i) {
        FunctionEntry entry;
        reader.readStruct(entry);

        auto name = resolveString(strings, entry.nameStringID);
        if (name.empty()) continue;

        Identifier id(name);
        if (!symbolTable.containsInScope(id, virtualScope)) {
            SymbolID symId = symbolTable.declareExternalSymbol(id, SymbolKind::Function,
                                              virtualScope, i, moduleUUID);
            if (mlibCache && entry.signatureTypeID < parsedTypeRefs.size()) {
                const Type* sigType = parsedTypeRefs[entry.signatureTypeID];
                if (sigType) {
                    mlibCache->registerType(symId, sigType);
                    std::cout << "[MLibLoader] Registered signature for " << name << "\n";
                }
            }
        }
    }
}

void ModuleLoader::registerTypes(const std::vector<uint8_t>& fileData,
                                 uint64_t sectionOffset,
                                 uint64_t sectionSize,
                                 ScopeID virtualScope,
                                 const std::vector<char>& strings,
                                 const uint8_t moduleUUID[16],
                                 const std::vector<const Type*>& parsedTypeRefs,
                                 std::vector<std::pair<SymbolID, uint32_t>>& pendingSignatures) {
    if (sectionSize == 0) return;

    BinaryReader reader(fileData.data() + sectionOffset, sectionSize);
    
    // 1. Namespaces
    uint32_t nsCount = reader.readU32();
    std::cout << "[MLibLoader] Loading " << nsCount << " namespaces\n";
    for (uint32_t i = 0; i < nsCount; ++i) {
        NamespaceEntry entry;
        reader.readStruct(entry);
        
        auto name = resolveString(strings, entry.nameStringID);
        if (name.empty()) continue;
        std::cout << "[MLibLoader] Namespace: " << name << "\n";

        Identifier id(name);
        if (!symbolTable.containsInScope(id, virtualScope)) {
            SymbolID nsId = symbolTable.declareExternalSymbol(id, SymbolKind::Module,
                                                              virtualScope, 0, moduleUUID);
            if (nsId != kInvalidSymbolID) {
                // Store virtualScope as the "body scope" so resolvePath can find
                // symbols like core::option::Option in the virtual scope
                symbolTable.getMutableSymbol(nsId).mlibSymbolID = static_cast<uint32_t>(virtualScope);
            }
        }
    }

    // 2. Types
    uint32_t typeCount = reader.readU32();
    std::cout << "[MLibLoader] Loading " << typeCount << " types\n";
    for (uint32_t i = 0; i < typeCount; ++i) {
        TypeEntry entry;
        reader.readStruct(entry);

        auto name = resolveString(strings, entry.nameStringID);
        if (name.empty()) continue;
        std::cout << "[MLibLoader] Type: " << name << "\n";

        Identifier id(name);
        if (!symbolTable.containsInScope(id, virtualScope)) {
            symbolTable.declareExternalSymbol(id, SymbolKind::Struct,
                                              virtualScope, i, moduleUUID);
        }
    }

    // 3. Traits
    uint32_t traitCount = reader.readU32();
    std::cout << "[MLibLoader] Loading " << traitCount << " traits\n";
    for (uint32_t i = 0; i < traitCount; ++i) {
        TraitEntry entry;
        reader.readStruct(entry);

        for (uint16_t j = 0; j < entry.genericParamCount; ++j) {
            TraitGenericParam gp;
            reader.readStruct(gp);
        }
        for (uint16_t j = 0; j < entry.methodCount; ++j) {
            TraitMethodEntry method;
            reader.readStruct(method);
            auto methodName = resolveString(strings, method.nameStringID);
            std::cout << "[MLibLoader]   Method: " << methodName << " (sigType=" << method.signatureTypeID << ")\n";
        }

        auto name = resolveString(strings, entry.nameStringID);
        std::cout << "[MLibLoader] Loaded Trait: " << name << " with " << entry.methodCount << " methods.\n";
        if (name.empty()) continue;
        std::cout << "[MLibLoader] Trait: " << name << "\n";

        Identifier id(name);
        if (!symbolTable.containsInScope(id, virtualScope)) {
            symbolTable.declareExternalSymbol(id, SymbolKind::Trait,
                                              virtualScope, i, moduleUUID);
        }
    }

    // 4. Functions
    std::cout << "[MLibLoader] Loading Functions...\n";
    uint32_t funcCount = reader.readU32();
    std::cout << "[MLibLoader] Found " << funcCount << " functions\n";
    for (uint32_t i = 0; i < funcCount; ++i) {
        FunctionEntry entry;
        reader.readStruct(entry);
        
        auto name = resolveString(strings, entry.nameStringID);
        if (name.empty()) continue;
        std::cout << "[MLibLoader] Function: " << name << "\n";

        Identifier id(name);
        if (!symbolTable.containsInScope(id, virtualScope)) {
            SymbolID symId = symbolTable.declareExternalSymbol(id, SymbolKind::Function,
                                              virtualScope, i, moduleUUID);
            if (mlibCache) {
                if (!parsedTypeRefs.empty() && entry.signatureTypeID < parsedTypeRefs.size()) {
                    const Type* sigType = parsedTypeRefs[entry.signatureTypeID];
                    if (sigType) {
                        mlibCache->registerType(symId, sigType);
                        std::cout << "[MLibLoader] Registered signature for " << name << " symId=" << symId << " type=" << sigType->toString() << "\n";
                    }
                } else {
                    pendingSignatures.push_back({symId, entry.signatureTypeID});
                }
            }
        }
    }
}

void ModuleLoader::registerImpls(const std::vector<uint8_t>& fileData,
                                  uint64_t sectionOffset,
                                  uint64_t sectionSize,
                                  ScopeID virtualScope,
                                  const std::vector<char>& strings,
                                  const uint8_t moduleUUID[16],
                                  const std::vector<const Type*>& parsedTypeRefs) {
    if (sectionSize == 0) return;

    BinaryReader reader(fileData.data() + sectionOffset, sectionSize);
    uint32_t count = reader.readU32();
    for (uint32_t i = 0; i < count; ++i) {
        ImplEntry entry;
        reader.readStruct(entry);

        std::cout << "[MLibLoader] Impl entry " << i
                  << " selfTypeRefID=" << entry.selfTypeRefID
                  << " traitRefID=" << entry.traitRefID
                  << " genericParamCount=" << entry.genericParamCount
                  << " methodCount=" << entry.methodCount
                  << " associatedTypeCount=" << entry.associatedTypeCount
                  << " boundCount=" << entry.boundCount
                  << " payloadSize=" << entry.payloadSize << "\n";
    }
}

void ModuleLoader::loadMacroMetadata(const std::vector<uint8_t>& fileData,
                                     uint64_t sectionOffset, uint64_t sectionSize,
                                     const std::vector<char>& strings,
                                     std::string_view moduleName) {
    if (!macroRegistry) return;
    if (sectionSize == 0) return;

    BinaryReader reader(fileData.data() + sectionOffset, sectionSize);
    uint32_t count = reader.readU32();
    for (uint32_t i = 0; i < count; ++i) {
        std::string macroName = reader.readString();
        std::string rawSource = reader.readString();

        // 1. Replace $crate with the actual moduleName
        std::string replacedSource;
        const std::string crateStr = "$crate";
        size_t pos = 0;
        size_t lastPos = 0;
        while ((pos = rawSource.find(crateStr, lastPos)) != std::string::npos) {
            replacedSource += rawSource.substr(lastPos, pos - lastPos);
            replacedSource += moduleName;
            lastPos = pos + crateStr.length();
        }
        replacedSource += rawSource.substr(lastPos);

        injectedStrings_.push_back(std::move(replacedSource));
        Lexer lexer(injectedStrings_.back());

        Parser parser(lexer, diag);
        auto node = parser.parseMacroDecl();
        
        // 3. Inject into MacroRegistry
        if (node) {
            auto* macroNode = static_cast<MacroDeclNode*>(node.release());
            macroRegistry->registerMacro(macroNode);
            // Ownership of macroNode is typically kept in an AST context, 
            // but for simplicity, we assume registerMacro keeps it or we leak it (if it doesn't take ownership).
            // In FDLang, usually the AST tree owns nodes, but macros are special.
            // Let's assume MacroRegistry takes ownership or we just let it leak for now since it's global.
        }
    }
}

void ModuleLoader::loadGenericMetadata(const std::vector<uint8_t>& fileData,
                                       uint64_t sectionOffset, uint64_t sectionSize,
                                       const std::vector<char>& strings,
                                       ScopeID virtualScope,
                                       std::string_view moduleName,
                                       const uint8_t moduleUUID[16]) {
    if (sectionSize == 0) return;

    BinaryReader reader(fileData.data() + sectionOffset, sectionSize);
    uint32_t count = reader.readU32();
    for (uint32_t i = 0; i < count; ++i) {
        GenericKind kind = static_cast<GenericKind>(reader.readU8());
        std::string name = reader.readString();
        std::string rawSource = reader.readString();
        std::cout << "[MLibLoader] Loading generic: " << name << " rawSource size=" << rawSource.size() << "\n";

        // 1. Replace $crate with actual moduleName
        std::string replacedSource;
        const std::string crateStr = "$crate";
        size_t pos = 0;
        size_t lastPos = 0;
        while ((pos = rawSource.find(crateStr, lastPos)) != std::string::npos) {
            replacedSource += rawSource.substr(lastPos, pos - lastPos);
            replacedSource += moduleName;
            lastPos = pos + crateStr.length();
        }
        replacedSource += rawSource.substr(lastPos);
        
        std::cout << "[MLibLoader] replacedSource: " << replacedSource << "\n";

        // 2. Parse the string into a DeclNode
        injectedStrings_.push_back(std::move(replacedSource));
        Lexer lexer(injectedStrings_.back());
        Parser parser(lexer, diag);
        
        std::unique_ptr<ItemNode> item;
        try {
            item = parser.parseDeclaration();
        } catch (const std::exception& e) {
            std::cout << "[CRITICAL] Parser crashed on generic: " << name << " with error: " << e.what() << "\n";
            std::cout << "String was:\n" << injectedStrings_.back() << "\n";
            throw;
        }
        
        if (auto decl = std::unique_ptr<DeclNode>(dynamic_cast<DeclNode*>(item.release()))) {
            // 3. Inject into virtual scope or Impl list
            if (kind == GenericKind::Impl) {
                auto* implNode = static_cast<ImplDeclNode*>(decl.get());
                auto optSyms = symbolTable.lookupInScope(name, virtualScope);
                if (!optSyms.empty()) {
                    injectedImpls_.push_back({optSyms[0], implNode});
                } else {
                    diag.error(SourceLocation::invalid(), "Could not find target struct '" + name + "' in .mlib");
                }
                Resolver tempResolver(symbolTable, diag);
                tempResolver.resolve(decl.get(), virtualScope);
                injectedGenerics_.push_back(std::move(decl));
            } else {
                // For Function, Struct, Enum: find the symbol in virtualScope and update its AST node
                auto optSyms = symbolTable.lookupInScope(name, virtualScope);
                if (!optSyms.empty()) {
                    for (auto id : optSyms) {
                        symbolTable.getMutableSymbol(id).decl = decl.get();
                        
                        auto* d = decl.get();
                        if (auto* fd = dynamic_cast<FunctionDeclNode*>(d)) { 
                            fd->symbolId = id; 
                            symbolTable.getMutableSymbol(id).kind = SymbolKind::Function; 
                            symbolTable.getFunctionInfo(id).borrowCheckStatus = BorrowCheckStatus::Checked;
                            std::cout << "[DEBUG] Updated kind to Function for id " << id << "\n"; 
                        }
                        else if (auto* ext = dynamic_cast<ExternDeclNode*>(d)) {
                            if (ext->func) {
                                ext->func->symbolId = id;
                                symbolTable.getMutableSymbol(id).kind = SymbolKind::Function;
                                symbolTable.getMutableSymbol(id).decl = ext->func.get();
                            }
                        }
                        else if (auto* sd = dynamic_cast<StructDeclNode*>(d)) { sd->symbolId = id; symbolTable.getMutableSymbol(id).kind = SymbolKind::Struct; std::cout << "[DEBUG] Updated kind to Struct for id " << id << "\n"; }
                        else if (auto* ed = dynamic_cast<EnumDeclNode*>(d)) { 
                            ed->symbolId = id; 
                            auto& msym = symbolTable.getMutableSymbol(id);
                            msym.kind = SymbolKind::Enum; 
                            msym.decl = ed;
                            ed->bodyScopeId = symbolTable.createScope(ScopeKind::Enum, kInvalidScopeID);
                            for (auto& variant : ed->variants) {
                                SymbolID varId = symbolTable.declareExternalSymbol(Identifier(variant->name), SymbolKind::EnumVariant, ed->bodyScopeId, 0, moduleUUID);
                                variant->symbolId = varId;
                            }
                            std::cout << "[DEBUG] Updated kind to Enum for id " << id << "\n"; 
                        }
                        else if (auto* td = dynamic_cast<TraitDeclNode*>(d)) { td->symbolId = id; symbolTable.getMutableSymbol(id).kind = SymbolKind::Trait; std::cout << "[DEBUG] Updated kind to Trait for id " << id << "\n"; }
                        else if (auto* ta = dynamic_cast<TypeAliasDeclNode*>(d)) { ta->symbolId = id; symbolTable.getMutableSymbol(id).kind = SymbolKind::TypeAlias; std::cout << "[DEBUG] Updated kind to TypeAlias for id " << id << "\n"; }
                    }
                } else {
                    SymbolKind skind = SymbolKind::Function; // default
                    if (kind == GenericKind::Function) skind = SymbolKind::Function;
                    else if (kind == GenericKind::Struct) skind = SymbolKind::Struct;
                    else if (kind == GenericKind::Enum) skind = SymbolKind::Enum;
                    else if (kind == GenericKind::Trait) skind = SymbolKind::Trait;
                    else if (kind == GenericKind::TypeAlias) skind = SymbolKind::TypeAlias;
                    
                    SymbolID newId = symbolTable.declareExternalSymbol(Identifier(name), skind, virtualScope, 0, moduleUUID);
                    auto* d = decl.get();
                    if (auto* fd = dynamic_cast<FunctionDeclNode*>(d)) {
                        fd->symbolId = newId;
                        symbolTable.getFunctionInfo(newId).borrowCheckStatus = BorrowCheckStatus::Checked;
                    }
                    else if (auto* ext = dynamic_cast<ExternDeclNode*>(d)) {
                        if (ext->func) ext->func->symbolId = newId;
                    }
                    else if (auto* sd = dynamic_cast<StructDeclNode*>(d)) sd->symbolId = newId;
                    else if (auto* ed = dynamic_cast<EnumDeclNode*>(d)) ed->symbolId = newId;
                    else if (auto* td = dynamic_cast<TraitDeclNode*>(d)) td->symbolId = newId;
                    else if (auto* ta = dynamic_cast<TypeAliasDeclNode*>(d)) ta->symbolId = newId;
                    
                    symbolTable.getMutableSymbol(newId).decl = d;
                }
                Resolver tempResolver(symbolTable, diag);
                tempResolver.resolve(decl.get(), virtualScope);
                injectedGenerics_.push_back(std::move(decl));
            }
}
    }
}

std::vector<const Type*> ModuleLoader::parseTypeRefs(const std::vector<uint8_t>& fileData,
                                                       uint64_t sectionOffset, uint64_t sectionSize,
                                                       const std::vector<char>& strings,
                                                       ScopeID virtualScope) {
    if (sectionSize == 0 || !typeContext) {
        std::cout << "[MLibLoader] parseTypeRefs empty sectionSize=" << sectionSize << " typeContext=" << typeContext << "\n";
        return {};
    }

    BinaryReader reader(fileData.data() + sectionOffset, sectionSize);
    uint32_t count = reader.readU32();
    std::cout << "[MLibLoader] parseTypeRefs count=" << count << "\n";
    std::vector<RawTypeRef> raws(count);
    
    for (uint32_t i = 0; i < count; ++i) {
        reader.readStruct(raws[i].record);
        if (raws[i].record.payloadSize > 0) {
            uint32_t numWords = raws[i].record.payloadSize / sizeof(uint32_t);
            raws[i].payload.resize(numWords);
            for (uint32_t j = 0; j < numWords; ++j) {
                raws[i].payload[j] = reader.readU32();
            }
        }
    }

    std::vector<const Type*> parsed(count, nullptr);
    
    std::function<const Type*(uint32_t)> resolve = [&](uint32_t id) -> const Type* {
        if (id >= count) return typeContext->getUnknown();
        if (parsed[id]) return parsed[id];
        
        const auto& raw = raws[id];
        const Type* result = typeContext->getUnknown();
        
        using namespace mlib;
        switch (static_cast<TypeRefKind>(raw.record.kind)) {
            case TypeRefKind::Primitive: {
                if (!raw.payload.empty()) {
                    result = typeContext->getPrimitive(static_cast<BuiltinKind>(raw.payload[0]));
                }
                break;
            }
            case TypeRefKind::Pointer: {
                if (!raw.payload.empty()) {
                    const Type* pointee = resolve(raw.payload[0]);
                    result = typeContext->create<PointerType>(pointee, (raw.record.flags & 1) != 0);
                }
                break;
            }
            case TypeRefKind::Reference: {
                if (!raw.payload.empty()) {
                    const Type* pointee = resolve(raw.payload[0]);
                    result = typeContext->create<ReferenceType>(pointee, (raw.record.flags & 1) != 0);
                }
                break;
            }
            case TypeRefKind::Array: {
                if (raw.payload.size() >= 2) {
                    const Type* elem = resolve(raw.payload[0]);
                    uint32_t len = raw.payload[1];
                    result = typeContext->create<ArrayType>(elem, len);
                }
                break;
            }
            case TypeRefKind::Slice: {
                if (!raw.payload.empty()) {
                    const Type* elem = resolve(raw.payload[0]);
                    result = typeContext->create<SliceType>(elem);
                }
                break;
            }
            case TypeRefKind::Tuple: {
                if (!raw.payload.empty()) {
                    uint32_t numElems = raw.payload[0];
                    std::vector<const Type*> elems;
                    for (uint32_t i = 0; i < numElems && (i + 1) < raw.payload.size(); ++i) {
                        elems.push_back(resolve(raw.payload[i + 1]));
                    }
                    result = typeContext->create<TupleType>(elems);
                }
                break;
            }
            case TypeRefKind::Function: {
                if (raw.payload.size() >= 3) {
                    const Type* retType = resolve(raw.payload[0]);
                    bool isVariadic = raw.payload[1] != 0;
                    uint32_t numParams = raw.payload[2];
                    std::vector<const Type*> paramTypes;
                    std::vector<std::string> paramNames(numParams, "");
                    for (uint32_t i = 0; i < numParams && (i + 3) < raw.payload.size(); ++i) {
                        paramTypes.push_back(resolve(raw.payload[i + 3]));
                    }
                    result = typeContext->getFunctionType(paramNames, paramTypes, retType, isVariadic);
                }
                break;
            }
            case TypeRefKind::GenericParam: {
                if (raw.payload.size() >= 2) {
                    uint32_t paramId = raw.payload[0];
                    uint32_t nameStringId = raw.payload[1];
                    auto gpName = resolveString(strings, nameStringId);
                    result = typeContext->create<GenericParamType>(paramId, std::string(gpName));
                }
                break;
            }
            case TypeRefKind::Named: {
                if (raw.payload.size() >= 2) {
                    uint32_t nameStringId = raw.payload[0];
                    uint32_t numArgs = raw.payload[1];
                    std::vector<const Type*> genericArgs;
                    for (uint32_t i = 0; i < numArgs && (i + 2) < raw.payload.size(); ++i) {
                        genericArgs.push_back(resolve(raw.payload[i + 2]));
                    }
                    
                    auto nameView = resolveString(strings, nameStringId);
                    if (!nameView.empty()) {
                        std::string name(nameView);
                        Identifier id(name);
                        
                        auto symIds = symbolTable.lookupInScope(id, virtualScope);
                        if (!symIds.empty()) {
                            SymbolID mappedSymId = symIds.front();
                            const auto& sym = symbolTable.getSymbol(mappedSymId);
                            if (sym.kind == SymbolKind::Struct) {
                                result = typeContext->getStructType(mappedSymId, std::move(genericArgs));
                            } else if (sym.kind == SymbolKind::Enum) {
                                result = typeContext->getEnumType(mappedSymId, std::move(genericArgs));
                            } else if (sym.kind == SymbolKind::Trait) {
                                result = typeContext->getTraitType(mappedSymId);
                            } else {
                                std::cout << "[MLibLoader] Named type '" << name << "' found but kind is " << (int)sym.kind << "\n";
                                result = typeContext->getUnknown();
                            }
                        } else {
                            std::cout << "[MLibLoader] Named type '" << name << "' NOT FOUND in virtualScope " << virtualScope << "\n";
                            result = typeContext->getUnknown();
                        }
                    } else {
                        result = typeContext->getUnknown();
                    }
                }
                break;
            }
            default:
                break; 
        }
        
        if (result && result->getKind() == TypeKind::Unknown) {
            std::cout << "[MLibLoader] parseTypeRefs entry " << id << " resolved to unknown! Kind=" << (int)raw.record.kind << "\n";
        }

        
        parsed[id] = result;
        return result;
    };

    for (uint32_t i = 0; i < count; ++i) {
        resolve(i);
    }

    return parsed;
}

} // namespace fl
