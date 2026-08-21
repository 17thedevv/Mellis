#ifndef MELLIS_MLIB_DEPGRAPHBUILDER_H
#define MELLIS_MLIB_DEPGRAPHBUILDER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/BinaryReader.h"
#include <vector>
#include <unordered_map>
#include <unordered_set>
#include <cstdint>

namespace fl {
namespace mlib {

// DepGraphBuilder: records directed call edges (callerID -> calleeID).
// Section layout: [Version(u32)] [EdgeCount(u32)] [DepEdge * EdgeCount]
class DepGraphBuilder {
public:
    // Record that callerID calls calleeID.
    void addEdge(uint32_t callerID, uint32_t calleeID);

    void serialize(BinaryWriter& writer) const;
    size_t getEdgeCount() const { return edges.size(); }

private:
    std::vector<DepEdge> edges;
};

// DepGraphReader: parses a Dependency Graph section and answers
// "which functions must be rebuilt if X changes?" queries.
class DepGraphReader {
public:
    DepGraphReader(const uint8_t* data, size_t size);
    void parse();

    // Returns all FunctionIDs that transitively depend on changedID.
    // i.e. all callers (direct + indirect) that must be rebuilt.
    std::unordered_set<uint32_t> getDependents(uint32_t changedID) const;

    size_t getEdgeCount() const { return edgeCount; }

private:
    const uint8_t* sectionData;
    size_t sectionSize;
    size_t edgeCount = 0;

    // reverseGraph[calleeID] = {callerID, ...} (reverse edges for traversal)
    std::unordered_map<uint32_t, std::vector<uint32_t>> reverseGraph;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_DEPGRAPHBUILDER_H
