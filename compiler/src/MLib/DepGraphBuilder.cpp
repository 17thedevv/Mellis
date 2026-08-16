#include "mellis/MLib/DepGraphBuilder.h"
#include <queue>
#include <stdexcept>

namespace fl {
namespace mlib {

// ──────────────────────────────────────────────────────────────────────────────
// DepGraphBuilder
// ──────────────────────────────────────────────────────────────────────────────

void DepGraphBuilder::addEdge(uint32_t callerID, uint32_t calleeID) {
    DepEdge e;
    e.callerID = callerID;
    e.calleeID = calleeID;
    edges.push_back(e);
}

void DepGraphBuilder::serialize(BinaryWriter& writer) const {
    writer.writeU32(1); // Version
    writer.writeU32(static_cast<uint32_t>(edges.size()));
    for (const auto& e : edges) {
        writer.writeStruct(e);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DepGraphReader
// ──────────────────────────────────────────────────────────────────────────────

DepGraphReader::DepGraphReader(const uint8_t* data, size_t size)
    : sectionData(data), sectionSize(size) {}

void DepGraphReader::parse() {
    BinaryReader reader(sectionData, sectionSize);

    uint32_t version = reader.readU32();
    if (version != 1) {
        throw std::runtime_error("Unsupported DepGraph Section version");
    }

    uint32_t count = reader.readU32();
    edgeCount = count;

    for (uint32_t i = 0; i < count; ++i) {
        DepEdge e;
        reader.readStruct(e);
        // Build reverse graph: callee -> [callers...]
        reverseGraph[e.calleeID].push_back(e.callerID);
    }
}

std::unordered_set<uint32_t> DepGraphReader::getDependents(uint32_t changedID) const {
    // BFS over the reverse graph starting from changedID
    std::unordered_set<uint32_t> visited;
    std::queue<uint32_t> queue;

    queue.push(changedID);

    while (!queue.empty()) {
        uint32_t current = queue.front();
        queue.pop();

        auto it = reverseGraph.find(current);
        if (it == reverseGraph.end()) continue;

        for (uint32_t caller : it->second) {
            if (visited.insert(caller).second) {
                queue.push(caller);
            }
        }
    }

    return visited;
}

} // namespace mlib
} // namespace fl
