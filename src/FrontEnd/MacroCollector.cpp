#include "mellis/FrontEnd/MacroCollector.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/DeclNode.h"

namespace fl {

void MacroCollector::visit(ProgramNode& node) {
    for (const auto& item : node.items) {
        if (item) {
            item->accept(*this);
        }
    }
}

void MacroCollector::visit(ModDeclNode& node) {
    for (const auto& decl : node.decls) {
        if (decl) {
            decl->accept(*this);
        }
    }
}

void MacroCollector::visit(MacroDeclNode& node) {
    registry_.registerMacro(&node);
}

} // namespace fl
