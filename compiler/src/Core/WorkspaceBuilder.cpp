#include "mellis/Core/WorkspaceBuilder.h"
#include "mellis/Core/DependencyScanner.h"
#include "mellis/Core/HashUtils.h"
#include "mellis/Core/SourceLocation.h"
#include "mellis/Support/Diagnostic.h"
#include <filesystem>
#include <fstream>
#include <sstream>
#include <iostream>
#include <cstring>

namespace fl {

// =============================================================================
// MLibHeader fast-read constants
// We only need to read the `moduleHash` field from the header.
// Layout (packed, little-endian):
//   [0 ]  char magic[4]             = "MLIB"
//   [4 ]  uint16_t formatVersion
//   [6 ]  uint16_t compilerVersion
//   [8 ]  char targetTriple[64]
//   [72]  uint8_t moduleUUID[16]
//   [88]  uint64_t moduleHash       ← we read 8 bytes here
// =============================================================================
static constexpr size_t kMLibHashOffset = 4 + 2 + 2 + 64 + 16; // = 88

// Read only the moduleHash from a .mlib file — no heap allocation for the full file.
// Returns 0 if file doesn't exist, can't be read, or magic bytes mismatch.
static uint64_t readMLibHash(const std::string& path) noexcept {
    std::ifstream f(path, std::ios::binary);
    if (!f.is_open()) return 0;

    // Check magic "MLIB"
    char magic[4];
    if (!f.read(magic, 4)) return 0;
    if (magic[0] != 'M' || magic[1] != 'L' || magic[2] != 'I' || magic[3] != 'B') return 0;

    // Seek to hash offset
    f.seekg(static_cast<std::streamoff>(kMLibHashOffset), std::ios::beg);
    if (!f.good()) return 0;

    // Read 8 bytes little-endian
    uint8_t buf[8];
    if (!f.read(reinterpret_cast<char*>(buf), 8)) return 0;

    uint64_t hash = 0;
    for (int i = 7; i >= 0; --i) {
        hash = (hash << 8) | buf[i];
    }
    return hash;
}

// Patch the moduleHash field in an existing .mlib file's header in-place.
// Seeks to byte offset 88 and writes exactly 8 bytes — zero heap allocation.
// Returns true on success.
static bool writeModuleHashToMLib(const std::string& path, uint64_t hash) noexcept {
    std::fstream f(path, std::ios::binary | std::ios::in | std::ios::out);
    if (!f.is_open()) return false;

    char magic[4];
    if (!f.read(magic, 4)) return false;
    if (magic[0] != 'M' || magic[1] != 'L' || magic[2] != 'I' || magic[3] != 'B') return false;

    f.seekp(static_cast<std::streamoff>(kMLibHashOffset), std::ios::beg);
    if (!f.good()) return false;

    uint8_t buf[8];
    for (int i = 0; i < 8; ++i) buf[i] = static_cast<uint8_t>((hash >> (i * 8)) & 0xFF);
    f.write(reinterpret_cast<const char*>(buf), 8);
    return f.good();
}


// =============================================================================
// WorkspaceBuilder Implementation
// =============================================================================

WorkspaceBuilder::WorkspaceBuilder(DiagnosticEngine& diag, const std::string& projectRoot)
    : diag(diag), projectRoot(projectRoot) {
}

std::string WorkspaceBuilder::resolveModulePath(const std::string& moduleName) const {
    namespace fs = std::filesystem;
    std::string path = moduleName;
    size_t pos = 0;
    while ((pos = path.find("::", pos)) != std::string::npos) {
        path.replace(pos, 2, "/");
        pos += 1;
    }
    fs::path fullPath = fs::path(projectRoot) / (path + ".ms");
    return fullPath.string();
}

std::string WorkspaceBuilder::resolveOutputPath(const std::string& moduleName,
                                                  const std::string& outputDir) const {
    namespace fs = std::filesystem;
    // Flatten "::" → "." for the output filename, e.g. "std::io" → "std.io.mlib"
    std::string name = moduleName;
    size_t pos = 0;
    while ((pos = name.find("::", pos)) != std::string::npos) {
        name.replace(pos, 2, ".");
        pos += 1;
    }
    return (fs::path(outputDir) / (name + ".mlib")).string();
}

bool WorkspaceBuilder::scanModuleRecursive(const std::string& moduleName,
                                            std::unordered_set<std::string>& visited) {
    if (visited.count(moduleName)) return true;
    visited.insert(moduleName);
    depGraph.addModule(moduleName);

    std::string filePath = resolveModulePath(moduleName);
    std::ifstream file(filePath);
    if (!file.is_open()) {
        // External .mlib — no local source, treat as having no local deps.
        return true;
    }

    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string sourceCode = buffer.str();

    auto deps = DependencyScanner::scanDependencies(sourceCode);
    for (const auto& dep : deps) {
        depGraph.addDependency(moduleName, dep);
        if (!scanModuleRecursive(dep, visited)) return false;
    }
    return true;
}

bool WorkspaceBuilder::buildGraphFromEntry(const std::string& entryModuleName) {
    std::unordered_set<std::string> visited;
    if (!scanModuleRecursive(entryModuleName, visited)) return false;

    try {
        buildOrder = depGraph.topologicalSort();
        return true;
    } catch (const std::runtime_error& e) {
        diag.error(SourceLocation::invalid(),
                   std::string("Workspace build failed: ") + e.what());
        return false;
    }
}

std::vector<std::string> WorkspaceBuilder::getBuildOrder() const {
    return buildOrder;
}

// =============================================================================
// computeBuildPlan — the core of Incremental Build.
//
// Algorithm:
//   Duyệt buildOrder theo thứ tự Topological (dependencies trước, consumers sau).
//   Với mỗi module:
//     1. Đọc source code từ .ms file.
//     2. Thu thập currentHash của các dependency (đã tính ở bước trước).
//     3. Tính currentHash = calculateModuleHash(src, depHashes).
//     4. Đọc cachedHash từ .mlib hiện có (O(1) — chỉ đọc 96 bytes đầu file).
//     5. needsRebuild = (currentHash != cachedHash).
//
// Tính chất Tuyết lở (Snowball):
//   Vì calculateModuleHash() trộn cả dep hashes vào,
//   nếu dep thay đổi → dep.currentHash thay đổi → consumer.currentHash thay đổi
//   → consumer.needsRebuild = true, dù consumer.ms không đổi.
// =============================================================================
std::vector<ModuleBuildStatus> WorkspaceBuilder::computeBuildPlan(const std::string& outputDir) {
    // Ensure output directory exists
    namespace fs = std::filesystem;
    std::error_code ec;
    fs::create_directories(outputDir, ec); // Ignore error if already exists

    // Map: moduleName → its computed currentHash (filled as we iterate topologically)
    std::unordered_map<std::string, uint64_t> computedHashes;

    std::vector<ModuleBuildStatus> plan;
    plan.reserve(buildOrder.size());

    for (const auto& moduleName : buildOrder) {
        ModuleBuildStatus status;
        status.moduleName = moduleName;
        status.sourceFile = resolveModulePath(moduleName);
        status.outputFile = resolveOutputPath(moduleName, outputDir);

        // Step 1: Read source code
        std::string sourceCode;
        std::ifstream srcFile(status.sourceFile);
        if (srcFile.is_open()) {
            std::stringstream buf;
            buf << srcFile.rdbuf();
            sourceCode = buf.str();
        }
        // If source file doesn't exist, treat as external .mlib — hash = 0
        // (external libs are pre-built and not rebuilt by us)

        // Step 2: Gather dependency hashes (already computed since we process in topo order)
        std::vector<uint64_t> depHashes;
        const auto& deps = depGraph.getDependencies(moduleName);
        for (const auto& dep : deps) {
            auto it = computedHashes.find(dep);
            if (it != computedHashes.end()) {
                depHashes.push_back(it->second);
            }
            // If dep not found in computedHashes, it might be external — skip
        }

        // Step 3: Compute current hash (Snowball effect built-in)
        if (sourceCode.empty()) {
            // External or missing — use cached hash as-is to avoid spurious rebuilds
            status.currentHash = readMLibHash(status.outputFile);
        } else {
            status.currentHash = calculateModuleHash(sourceCode, depHashes);
        }
        computedHashes[moduleName] = status.currentHash;

        // Step 4: Read cached hash (fast — reads only 96 bytes from .mlib header)
        status.cachedHash = readMLibHash(status.outputFile);

        // Step 5: Decide
        status.needsRebuild = (status.currentHash != status.cachedHash);

        plan.push_back(std::move(status));
    }

    return plan;
}

std::vector<ModuleBuildStatus> WorkspaceBuilder::getDirtyModules(const std::string& outputDir) {
    auto plan = computeBuildPlan(outputDir);
    std::vector<ModuleBuildStatus> dirty;
    for (auto& s : plan) {
        if (s.needsRebuild) dirty.push_back(std::move(s));
    }
    return dirty;
}

bool WorkspaceBuilder::markModuleBuilt(const std::string& mlibPath, uint64_t hash) noexcept {
    return writeModuleHashToMLib(mlibPath, hash);
}

uint64_t WorkspaceBuilder::readCachedHash(const std::string& mlibPath) noexcept {
    return readMLibHash(mlibPath);
}

} // namespace fl
