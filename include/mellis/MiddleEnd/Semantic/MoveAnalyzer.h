#pragma once

#include "mellis/MiddleEnd/Semantic/DataflowEngine.h"
#include "mellis/MiddleEnd/Place.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

enum class MoveState {
    Valid,
    Moved,
    PartiallyMoved
};

struct MoveStateData {
    std::unordered_map<std::string, Place> placeMap;
    std::unordered_map<std::string, MoveState> stateMap;

    bool operator==(const MoveStateData& other) const {
        return stateMap == other.stateMap;
    }
    bool operator!=(const MoveStateData& other) const { return !(*this == other); }
};

class MoveAnalyzer : public DataflowPass<MoveStateData> {
    const mvir::Module* module_;
    DiagnosticEngine& diag_;
    std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap_;
    bool hasError_ = false;

public:
    MoveAnalyzer(const mvir::Module* module, DiagnosticEngine& diag, 
                 std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap)
        : module_(module), diag_(diag), closureStorageMap_(closureStorageMap) {}

    bool isCopy(const Type* t) const {
        if (!t) return false;
        if (auto* closureTy = dynamic_cast<const ClosureType*>(t)) {
            if (closureStorageMap_.find(closureTy) != closureStorageMap_.end() && 
                closureStorageMap_.at(closureTy) == ClosureStorageKind::Heap) {
                return false; // Heap closures are Move-Only
            }
        }
        return t->isCopy();
    }

    bool analyzeFunction(const mvir::Function& func) {
        hasError_ = false;
        run(func);
        return !hasError_;
    }

    void transferInstruction(const mvir::Instruction& inst, MoveStateData& state) override;
    void transferTerminator(const mvir::Terminator& term, MoveStateData& state) override;
    bool merge(MoveStateData& dest, const MoveStateData& src) override;
    void initEntryState(const mvir::Function& func, MoveStateData& state) override;

private:
    Place resolvePlace(const mvir::Operand& op, const MoveStateData& state) const;
    void checkAccess(const Place& place, SourceLocation loc, const MoveStateData& state);
};

} // namespace fl
