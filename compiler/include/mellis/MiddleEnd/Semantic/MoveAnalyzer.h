#pragma once

#include "mellis/MiddleEnd/Semantic/DataflowEngine.h"
#include "mellis/MiddleEnd/Place.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/MiddleEnd/TraitSolver.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include <unordered_map>

namespace fl {
class TraitSolver;

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
    TraitSolver* solver_;
    SymbolTable* symTable_;
    bool hasError_ = false;

public:
    MoveAnalyzer(const mvir::Module* module, DiagnosticEngine& diag, 
                 std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap,
                 TraitSolver* solver = nullptr, SymbolTable* symTable = nullptr)
        : module_(module), diag_(diag), closureStorageMap_(closureStorageMap), 
          solver_(solver), symTable_(symTable) {}

    bool isCopy(const Type* t) const;

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
