#pragma once

#include "mellis/MiddleEnd/Semantic/DataflowEngine.h"
#include "mellis/MiddleEnd/Place.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

enum class InitState {
    Uninitialized,
    Initialized
};

struct InitStateData {
    std::unordered_map<std::string, Place> placeMap;
    std::unordered_map<std::string, InitState> initStateMap;

    bool operator==(const InitStateData& other) const {
        return initStateMap == other.initStateMap;
    }
    bool operator!=(const InitStateData& other) const { return !(*this == other); }
};

class InitializationAnalyzer : public DataflowPass<InitStateData> {
    const mvir::Module* module_;
    DiagnosticEngine& diag_;
    bool hasError_ = false;

public:
    InitializationAnalyzer(const mvir::Module* module, DiagnosticEngine& diag)
        : module_(module), diag_(diag) {}

    bool analyzeFunction(const mvir::Function& func) {
        hasError_ = false;
        run(func);
        return !hasError_;
    }

    void transferInstruction(const mvir::Instruction& inst, InitStateData& state) override;
    void transferTerminator(const mvir::Terminator& term, InitStateData& state) override;
    bool merge(InitStateData& dest, const InitStateData& src) override;
    void initEntryState(const mvir::Function& func, InitStateData& state) override;

private:
    Place resolvePlace(const mvir::Operand& op, const InitStateData& state) const;
    void checkAccess(const Place& place, SourceLocation loc, const InitStateData& state);
};

} // namespace fl
