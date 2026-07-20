#include "mellis/FrontEnd/ImportResolver.h"

namespace fl {

void ImportResolver::visit(ProgramNode& node) {
    for (const auto& item : node.items) {
        if (item) {
            item->accept(*this);
        }
    }
}

void ImportResolver::visit(ModDeclNode& node) {
    for (const auto& decl : node.decls) {
        if (decl) {
            decl->accept(*this);
        }
    }
}

void ImportResolver::visit(UseDeclNode& node) {
    // Phase 1.3: ImportResolver stub
    // Resolving imports/exports and creating module structure will be implemented here.
    (void)node;
}

} // namespace fl
