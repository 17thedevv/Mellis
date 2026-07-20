#include "mellis/Core/WorkspaceBuilder.h"
#include "mellis/Core/DependencyScanner.h"
#include "mellis/Support/Diagnostic.h"
#include <iostream>
#include <cassert>
#include <fstream>
#include <filesystem>

using namespace fl;

void testDependencyScanner() {
    std::string source = R"(
        // Some comments
        /* block comment */
        use std::io;
        use math::algebra as alg;
        
        fn main() {
            use internal::helper; // Should not be reached due to early exit!
        }
    )";
    
    auto deps = DependencyScanner::scanDependencies(source);
    assert(deps.size() == 2);
    assert(deps[0] == "std::io");
    assert(deps[1] == "math::algebra");
}

void testModuleDepGraphValid() {
    ModuleDepGraph graph;
    // A -> B, A -> C, B -> C
    graph.addDependency("A", "B");
    graph.addDependency("A", "C");
    graph.addDependency("B", "C");
    
    auto order = graph.topologicalSort();
    assert(order.size() == 3);
    // C must be before B, B must be before A
    // Expected: C, B, A
    assert(order[0] == "C");
    assert(order[1] == "B");
    assert(order[2] == "A");
}

void testModuleDepGraphCycle() {
    ModuleDepGraph graph;
    graph.addDependency("A", "B");
    graph.addDependency("B", "C");
    graph.addDependency("C", "A");
    
    bool caught = false;
    try {
        graph.topologicalSort();
    } catch (const std::runtime_error& e) {
        caught = true;
        std::string msg = e.what();
        assert(msg.find("Circular dependency detected") != std::string::npos);
        assert(msg.find("A -> B -> C -> A") != std::string::npos || 
               msg.find("B -> C -> A -> B") != std::string::npos ||
               msg.find("C -> A -> B -> C") != std::string::npos);
    }
    assert(caught);
}

void testWorkspaceBuilder() {
    namespace fs = std::filesystem;
    fs::create_directories("test_workspace");
    
    // main.ms -> a.ms, b.ms
    std::ofstream("test_workspace/main.ms") << "use a;\nuse b;\nfn main() {}";
    // a.ms -> c.ms
    std::ofstream("test_workspace/a.ms") << "use c;\nfn a() {}";
    // b.ms -> c.ms
    std::ofstream("test_workspace/b.ms") << "use c;\nfn b() {}";
    // c.ms -> no deps
    std::ofstream("test_workspace/c.ms") << "fn c() {}";
    
    DiagnosticEngine diag;
    WorkspaceBuilder builder(diag, "test_workspace");
    bool ok = builder.buildGraphFromEntry("main");
    assert(ok);
    
    auto order = builder.getBuildOrder();
    assert(order.size() == 4);
    assert(order[0] == "c");
    assert(order.back() == "main");
    
    // Cleanup
    fs::remove_all("test_workspace");
}

int main() {
    testDependencyScanner();
    std::cout << "testDependencyScanner passed" << std::endl;
    
    testModuleDepGraphValid();
    std::cout << "testModuleDepGraphValid passed" << std::endl;
    
    testModuleDepGraphCycle();
    std::cout << "testModuleDepGraphCycle passed" << std::endl;
    
    testWorkspaceBuilder();
    std::cout << "testWorkspaceBuilder passed" << std::endl;
    
    std::cout << "All Workspace Builder tests passed!" << std::endl;
    return 0;
}
