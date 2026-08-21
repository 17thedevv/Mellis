#include "mellis/Core/ModuleDepGraph.h"
#include <stdexcept>
#include <algorithm>

namespace fl {

void ModuleDepGraph::addModule(const std::string& moduleName) {
    if (adjList.find(moduleName) == adjList.end()) {
        adjList[moduleName] = std::vector<std::string>();
    }
}

void ModuleDepGraph::addDependency(const std::string& moduleName, const std::string& dependencyName) {
    // Ensure both exist in graph
    addModule(moduleName);
    addModule(dependencyName);
    
    // Add directed edge (moduleName -> dependencyName)
    // Avoid duplicate edges
    auto& deps = adjList[moduleName];
    if (std::find(deps.begin(), deps.end(), dependencyName) == deps.end()) {
        deps.push_back(dependencyName);
    }
}

static const std::vector<std::string> kEmptyDeps;

const std::vector<std::string>& ModuleDepGraph::getDependencies(const std::string& moduleName) const {
    auto it = adjList.find(moduleName);
    if (it != adjList.end()) return it->second;
    return kEmptyDeps;
}

std::vector<std::string> ModuleDepGraph::topologicalSort() const {
    std::unordered_map<std::string, NodeState> states;
    for (const auto& pair : adjList) {
        states[pair.first] = NodeState::Unvisited;
    }

    std::vector<std::string> order;
    std::vector<std::string> path;

    for (const auto& pair : adjList) {
        if (states[pair.first] == NodeState::Unvisited) {
            if (!dfs(pair.first, states, path, order)) {
                // Cycle detected, exception is thrown inside dfs
            }
        }
    }

    // DFS yields reverse topological sort (deepest dependencies first),
    // which is exactly what we want for bottom-up compilation!
    // order = [C, B, A] where A -> B -> C.
    return order;
}

bool ModuleDepGraph::dfs(const std::string& node, 
                         std::unordered_map<std::string, NodeState>& states, 
                         std::vector<std::string>& path, 
                         std::vector<std::string>& order) const {
    states[node] = NodeState::Visiting;
    path.push_back(node);

    auto it = adjList.find(node);
    if (it != adjList.end()) {
        for (const std::string& neighbor : it->second) {
            if (states[neighbor] == NodeState::Visiting) {
                // Cycle detected
                std::string errorMsg = "Circular dependency detected: ";
                bool inCycle = false;
                for (const auto& p : path) {
                    if (p == neighbor) inCycle = true;
                    if (inCycle) {
                        errorMsg += p + " -> ";
                    }
                }
                errorMsg += neighbor;
                throw std::runtime_error(errorMsg);
            } else if (states[neighbor] == NodeState::Unvisited) {
                if (!dfs(neighbor, states, path, order)) return false;
            }
        }
    }

    states[node] = NodeState::Visited;
    path.pop_back();
    order.push_back(node); // Append to order after all dependencies are resolved
    return true;
}

} // namespace fl
