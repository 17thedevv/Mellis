#include "mellis/Core/WorkspaceBuilder.h"
#include "mellis/Core/DependencyScanner.h"
#include "mellis/Core/SourceLocation.h"
#include "mellis/Support/Diagnostic.h"
#include <filesystem>
#include <fstream>
#include <sstream>
#include <iostream>

namespace fl {

WorkspaceBuilder::WorkspaceBuilder(DiagnosticEngine& diag, const std::string& projectRoot)
    : diag(diag), projectRoot(projectRoot) {
}

std::string WorkspaceBuilder::resolveModulePath(const std::string& moduleName) const {
    namespace fs = std::filesystem;
    // For now, map "std::io" to "std/io.ms" or just "std.ms".
    // For simplicity in this demo compiler, we replace "::" with "/" and append ".ms".
    std::string path = moduleName;
    size_t pos = 0;
    while ((pos = path.find("::", pos)) != std::string::npos) {
        path.replace(pos, 2, "/");
        pos += 1;
    }
    
    fs::path fullPath = fs::path(projectRoot) / (path + ".ms");
    return fullPath.string();
}

bool WorkspaceBuilder::scanModuleRecursive(const std::string& moduleName, std::unordered_set<std::string>& visited) {
    if (visited.count(moduleName)) {
        return true; // Already processed
    }
    visited.insert(moduleName);
    depGraph.addModule(moduleName);

    std::string filePath = resolveModulePath(moduleName);
    
    std::ifstream file(filePath);
    if (!file.is_open()) {
        // If file not found in local workspace, it might be an external .mlib library (like std.mlib).
        // For Dependency Graph, we just assume it has no local dependencies if not found.
        return true; 
    }

    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string sourceCode = buffer.str();

    auto deps = DependencyScanner::scanDependencies(sourceCode);
    for (const auto& dep : deps) {
        depGraph.addDependency(moduleName, dep);
        if (!scanModuleRecursive(dep, visited)) {
            return false;
        }
    }

    return true;
}

bool WorkspaceBuilder::buildGraphFromEntry(const std::string& entryModuleName) {
    std::unordered_set<std::string> visited;
    
    if (!scanModuleRecursive(entryModuleName, visited)) {
        return false;
    }

    try {
        buildOrder = depGraph.topologicalSort();
        return true;
    } catch (const std::runtime_error& e) {
        diag.error(SourceLocation::invalid(), std::string("Workspace build failed: ") + e.what());
        return false;
    }
}

std::vector<std::string> WorkspaceBuilder::getBuildOrder() const {
    return buildOrder;
}

} // namespace fl
