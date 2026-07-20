#pragma once

#include <string>
#include <unordered_map>
#include "mellis/AST/MacroNode.h"
#include "mellis/Core/Types.h"
#include "mellis/Support/Diagnostic.h"
#include <vector>

namespace fl {

enum class Visibility {
    Private,
    Public,
    Crate
};

enum class ExpansionStrategy {
    Declarative,
    Procedural
};

struct MacroDefinition {
    MacroID id;
    SymbolID symbol;
    ModuleID module;
    Visibility visibility;
    ExpansionStrategy strategy;
    const MacroDeclNode* templateAST;
};

class MacroRegistry {
public:
    MacroRegistry(DiagnosticEngine& diag) : diag_(diag), nextId_(1) {} // Start ID from 1

    // Register a macro declaration, returning its MacroID
    MacroID registerMacro(const MacroDeclNode* node);

    // Lookup a macro by MacroID
    const MacroDefinition* getMacro(MacroID id) const;

    // Lookup a macro by name
    MacroID findMacroByName(const std::string& name) const;

private:
    DiagnosticEngine& diag_;
    MacroID nextId_;
    std::vector<MacroDefinition> macros_; // Indexed by MacroID - 1
    std::unordered_map<std::string, MacroID> nameToId_;
};

} // namespace fl
