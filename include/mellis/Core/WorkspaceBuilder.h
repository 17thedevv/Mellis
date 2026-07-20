#ifndef MELLIS_CORE_WORKSPACE_BUILDER_H
#define MELLIS_CORE_WORKSPACE_BUILDER_H

#include "mellis/Core/ModuleDepGraph.h"
#include <string>
#include <vector>

namespace fl {

class DiagnosticEngine;

class WorkspaceBuilder {
public:
    explicit WorkspaceBuilder(DiagnosticEngine& diag, const std::string& projectRoot = ".");

    // Scans a specific entry module (e.g. "main"), discovers all its dependencies
    // in the project workspace, and builds the Dependency Graph.
    bool buildGraphFromEntry(const std::string& entryModuleName);

    // After building the graph, this returns the correct build order.
    std::vector<std::string> getBuildOrder() const;

private:
    DiagnosticEngine& diag;
    std::string projectRoot;
    ModuleDepGraph depGraph;
    std::vector<std::string> buildOrder;
    
    // Reads a module file (.ms), scans for dependencies, and adds to graph.
    // Returns false if file not found or cyclic dependency.
    bool scanModuleRecursive(const std::string& moduleName, std::unordered_set<std::string>& visited);

    std::string resolveModulePath(const std::string& moduleName) const;
};

} // namespace fl

#endif // MELLIS_CORE_WORKSPACE_BUILDER_H
