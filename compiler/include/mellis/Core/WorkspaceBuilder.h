#ifndef MELLIS_CORE_WORKSPACE_BUILDER_H
#define MELLIS_CORE_WORKSPACE_BUILDER_H

#include "mellis/Core/ModuleDepGraph.h"
#include <string>
#include <vector>
#include <unordered_map>

namespace fl {

class DiagnosticEngine;

// =============================================================================
// ModuleBuildStatus — describes whether a single module needs to be rebuilt.
//
// Produced by WorkspaceBuilder::computeBuildPlan() after comparing the
// newly-computed hash with the one stored in the existing .mlib file.
//
// The `currentHash` is the "Avalanche Hash" that encodes:
//   - This module's own source content.
//   - All direct and transitive dependency hashes (snowball effect).
// =============================================================================
struct ModuleBuildStatus {
    std::string moduleName;    // e.g. "math", "std::io"
    std::string sourceFile;    // Absolute path to .ms file (empty if external .mlib)
    std::string outputFile;    // Absolute path to target .mlib file
    uint64_t    currentHash;   // Hash computed now from source + dep chain
    uint64_t    cachedHash;    // Hash read from existing .mlib (0 = no cache)
    bool        needsRebuild;  // true if currentHash != cachedHash
};

class WorkspaceBuilder {
public:
    explicit WorkspaceBuilder(DiagnosticEngine& diag, const std::string& projectRoot = ".");

    // ── Phase 1: Build the Dependency Graph ────────────────────────────────────
    // Scans a specific entry module (e.g. "main"), discovers all its
    // dependencies in the project workspace, and builds the Dependency Graph.
    bool buildGraphFromEntry(const std::string& entryModuleName);

    // After building the graph, this returns the correct build order.
    std::vector<std::string> getBuildOrder() const;

    // ── Phase 2: Incremental Build Planning ────────────────────────────────────
    // After buildGraphFromEntry() succeeds, call this to compute which modules
    // actually need to be rebuilt.
    //
    // @param outputDir   Directory where .mlib files are stored (e.g. "build/cache/").
    //                    The output path for module "math" will be
    //                    "<outputDir>/math.mlib".
    // @return            List of ModuleBuildStatus, in topological order.
    //                    Modules with needsRebuild=false can be safely skipped.
    std::vector<ModuleBuildStatus> computeBuildPlan(const std::string& outputDir);

    // Convenience: returns only modules that need rebuilding (needsRebuild=true).
    // Useful for driving CompilerSession::compile() calls.
    std::vector<ModuleBuildStatus> getDirtyModules(const std::string& outputDir);

    // After CompilerSession::compile() succeeds for a module, call this to
    // seal the hash into the .mlib header so future builds can skip it.
    // @param mlibPath   Absolute path to the freshly-written .mlib file.
    // @param hash       The ModuleBuildStatus::currentHash from the build plan.
    // @return true if the patch succeeded.
    static bool markModuleBuilt(const std::string& mlibPath, uint64_t hash) noexcept;

    // Fast read of the stored hash from an existing .mlib (reads only 96 bytes).
    // Returns 0 if the file doesn't exist or is not a valid MLIB file.
    static uint64_t readCachedHash(const std::string& mlibPath) noexcept;

private:
    DiagnosticEngine& diag;
    std::string projectRoot;
    ModuleDepGraph depGraph;
    std::vector<std::string> buildOrder;

    // Reads a module file (.ms), scans for dependencies, and adds to graph.
    bool scanModuleRecursive(const std::string& moduleName,
                              std::unordered_set<std::string>& visited);

    std::string resolveModulePath(const std::string& moduleName) const;
    std::string resolveOutputPath(const std::string& moduleName,
                                   const std::string& outputDir) const;
};

} // namespace fl

#endif // MELLIS_CORE_WORKSPACE_BUILDER_H
