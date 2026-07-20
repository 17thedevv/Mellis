#include "mellis/FrontEnd/MacroRegistry.h"

namespace fl {

MacroID MacroRegistry::registerMacro(const MacroDeclNode* node) {
    if (!node) return kInvalidMacroID;

    auto it = nameToId_.find(node->name);
    if (it != nameToId_.end()) {
        diag_.error(node->loc, "Macro '" + node->name + "' da duoc dinh nghia truoc do.");
        // We could look up the old location, but let's just report the error for now.
        return kInvalidMacroID;
    }

    MacroID id = nextId_++;
    
    MacroDefinition def;
    def.id = id;
    def.symbol = kInvalidSymbolID;
    def.module = kInvalidModuleID;
    def.visibility = Visibility::Crate; // Default for now
    def.strategy = ExpansionStrategy::Declarative; // We only support declarative right now
    def.templateAST = node;

    macros_.push_back(def);
    nameToId_[node->name] = id;
    
    return id;
}

const MacroDefinition* MacroRegistry::getMacro(MacroID id) const {
    if (id == 0 || id == kInvalidMacroID || id > macros_.size()) {
        return nullptr;
    }
    return &macros_[id - 1];
}

MacroID MacroRegistry::findMacroByName(const std::string& name) const {
    auto it = nameToId_.find(name);
    if (it != nameToId_.end()) {
        return it->second;
    }
    return kInvalidMacroID;
}

} // namespace fl
