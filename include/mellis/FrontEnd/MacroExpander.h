#pragma once
#include "mellis/FrontEnd/ASTTransformer.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include "mellis/Support/Diagnostic.h"
#include <vector>

namespace fl {

struct ExpansionFrame {
    MacroID macroId;
    SourceLocation callSite;
};

class MacroExpander : public ASTTransformer {
public:
    MacroExpander(MacroRegistry& registry, DiagnosticEngine& diag)
        : registry_(registry), diag_(diag) {}

    void visit(MacroCallExpr& node) override;
    void visit(MacroCallStmt& node) override;
    // visit(MacroExpandForStmt& node) override; // To be implemented later if needed

private:
    std::unique_ptr<ASTNode> expandMacro(MacroID macroId, const SourceLocation& loc, const std::vector<MacroCallArgNode>& args);

    MacroRegistry& registry_;
    DiagnosticEngine& diag_;
    std::vector<ExpansionFrame> expansionStack_;
    ExpansionID nextExpansionId_ = 1;
};

} // namespace fl
