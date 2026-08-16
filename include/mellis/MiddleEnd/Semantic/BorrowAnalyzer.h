#pragma once

#include "mellis/MiddleEnd/Semantic/DataflowEngine.h"
#include "mellis/MiddleEnd/Place.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/MiddleEnd/LivenessAnalyzer.h"
#include "mellis/MiddleEnd/SymbolTable.h"

namespace fl {

struct Loan {
    size_t id;
    Place place;
    bool isMutable;
    std::string referenceId;
    SourceLocation loc;
};

struct BorrowStateData {
    std::vector<Loan> activeLoans;
    std::unordered_map<std::string, std::vector<Place>> placeMap;

    bool operator==(const BorrowStateData& other) const {
        if (activeLoans.size() != other.activeLoans.size()) return false;
        for (size_t i = 0; i < activeLoans.size(); ++i) {
            if (!(activeLoans[i].place == other.activeLoans[i].place && 
                  activeLoans[i].isMutable == other.activeLoans[i].isMutable)) return false;
        }
        return true;
    }
    bool operator!=(const BorrowStateData& other) const { return !(*this == other); }
};

class BorrowAnalyzer : public DataflowPass<BorrowStateData> {
    const mvir::Module* module_;
    DiagnosticEngine& diag_;
    SymbolTable& symTable_;
    bool hasError_ = false;
    size_t nextLoanId_ = 0;
    LivenessInfo liveness_;
    std::unordered_map<const mvir::Terminator*, const mvir::BasicBlock*> termToBlock_;

public:
    BorrowAnalyzer(const mvir::Module* module, DiagnosticEngine& diag, SymbolTable& symTable)
        : module_(module), diag_(diag), symTable_(symTable) {}

    bool analyzeFunction(const mvir::Function& func) {
        if (func.name.symbolId != 0) {
            symTable_.getFunctionInfo(func.name.symbolId).borrowCheckStatus = BorrowCheckStatus::Checking;
        }

        hasError_ = false;
        liveness_ = LivenessAnalyzer::computeLiveness(func);
        termToBlock_.clear();
        for (const auto& block : func.blocks) {
            if (block->terminator) {
                termToBlock_[block->terminator.get()] = block.get();
            }
        }
        run(func);

        if (func.name.symbolId != 0) {
            symTable_.getFunctionInfo(func.name.symbolId).borrowCheckStatus = hasError_ ? BorrowCheckStatus::Failed : BorrowCheckStatus::Checked;
        }

        return !hasError_;
    }

    void transferInstruction(const mvir::Instruction& inst, BorrowStateData& state) override;
    void transferTerminator(const mvir::Terminator& term, BorrowStateData& state) override;
    bool merge(BorrowStateData& dest, const BorrowStateData& src) override;
    void initEntryState(const mvir::Function& func, BorrowStateData& state) override;
    void run(const mvir::Function& func) {
        DataflowPass<BorrowStateData>::run(func);
    }

private:
    std::vector<Place> resolvePlace(const mvir::Operand& op, const BorrowStateData& state) const;
    void checkAccess(const Place& place, bool isMut, SourceLocation loc, const BorrowStateData& state);
    void issueLoan(const Place& place, bool isMut, const std::string& refId, SourceLocation loc, BorrowStateData& state);
};

} // namespace fl
