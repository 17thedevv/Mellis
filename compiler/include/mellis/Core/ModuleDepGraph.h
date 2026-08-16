#ifndef MELLIS_CORE_MODULEDEPGRAPH_H
#define MELLIS_CORE_MODULEDEPGRAPH_H

#include <string>
#include <vector>
#include <unordered_map>
#include <unordered_set>

namespace fl {

// Represents a directed acyclic graph (DAG) of modules/packages for workspace building.
class ModuleDepGraph {
public:
    // Adds a module to the graph (if not already present).
    void addModule(const std::string& moduleName);

    // Adds a directed dependency: `moduleName` depends on `dependencyName`.
    void addDependency(const std::string& moduleName, const std::string& dependencyName);

    // Returns the direct dependencies of a module (empty if none or not found).
    const std::vector<std::string>& getDependencies(const std::string& moduleName) const;

    // Performs a topological sort of the graph.
    // Returns the ordered list of modules (independent modules first).
    // If a cycle is detected, throws a std::runtime_error with the cycle path.
    std::vector<std::string> topologicalSort() const;

private:
    std::unordered_map<std::string, std::vector<std::string>> adjList;

    // Helper for DFS
    enum class NodeState {
        Unvisited,
        Visiting,
        Visited
    };
    bool dfs(const std::string& node, 
             std::unordered_map<std::string, NodeState>& states, 
             std::vector<std::string>& path, 
             std::vector<std::string>& order) const;
};

} // namespace fl

#endif // MELLIS_CORE_MODULEDEPGRAPH_H
