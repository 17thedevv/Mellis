#include "mellis/FrontEnd/MacroExpander.h"
#include "mellis/FrontEnd/PlaceholderReplacer.h"
#include "mellis/FrontEnd/ASTCloner.h"
#include <iostream>

namespace fl {

std::unique_ptr<ASTNode> MacroExpander::expandMacro(MacroID macroId, const SourceLocation& loc, const std::vector<MacroCallArgNode>& args) {
    std::cerr << "[DEBUG] expandMacro called with macroId=" << macroId << " at line " << loc.line << std::endl;
    if (macroId == kInvalidMacroID) {
        std::cerr << "[DEBUG] macroId is kInvalidMacroID, returning nullptr" << std::endl; // Error already reported by Resolver
        return nullptr;
    }

    // 1. Check for infinite recursion
    for (const auto& frame : expansionStack_) {
        if (frame.macroId == macroId) {
            std::cerr << "[DEBUG] Recursive macro expansion detected for macroId=" << macroId << std::endl;
            diag_.error(loc, "recursive macro expansion");
            return nullptr;
        }
    }

    // 2. Get MacroDefinition
    const MacroDefinition* def = registry_.getMacro(macroId);
    if (!def || !def->templateAST || !def->templateAST->body) {
        return nullptr;
    }

    // 3. Match arguments to parameters
    std::unordered_map<std::string, PlaceholderBinding> argMap;
    const auto& params = def->templateAST->params;
    
    // We assume MacroValidator already validated argument count,
    // so we can safely bind them.
    for (size_t i = 0; i < params.size(); ++i) {
        if (params[i].isVariadic) {
            std::vector<const ASTNode*> nodes;
            for (size_t j = i; j < args.size(); ++j) {
                nodes.push_back(args[j].node.get());
            }
            argMap[params[i].name] = PlaceholderBinding{true, nodes};
        } else {
            if (i < args.size()) {
                argMap[params[i].name] = PlaceholderBinding{false, {args[i].node.get()}};
            }
        }
    }

    // 4. Push frame
    expansionStack_.push_back({macroId, loc});

    // 5. Clone and Replace
    ASTCloner cloner;
    auto clonedBody = cloner.clone(*def->templateAST->body);

    ExpansionID expId = nextExpansionId_++;
    PlaceholderReplacer replacer(argMap, expId, diag_);
    auto expandedAST = replacer.transformNode(std::move(clonedBody));

    // 6. Recursively expand macros INSIDE the expanded AST!
    // Because the expanded AST might contain further MacroCallExprs.
    expandedAST = this->transformNode(std::move(expandedAST));

    // 7. Pop frame
    expansionStack_.pop_back();

    return expandedAST;
}

void MacroExpander::visit(MacroCallExpr& node) {
    std::cerr << "[DEBUG] MacroExpander::visit(MacroCallExpr) for " << node.name << " with id " << node.resolvedMacroId << std::endl;
    result_ = expandMacro(node.resolvedMacroId, node.loc, node.args);
}

void MacroExpander::visit(MacroCallStmt& node) {
    std::cerr << "[DEBUG] MacroExpander::visit(MacroCallStmt) for " << node.name << " with id " << node.resolvedMacroId << std::endl;
    result_ = expandMacro(node.resolvedMacroId, node.loc, node.args);
}

} // namespace fl
