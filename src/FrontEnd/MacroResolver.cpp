#include "mellis/FrontEnd/MacroResolver.h"
#include "mellis/AST/ASTNode.h"
#include <iostream>

namespace fl {

void MacroResolver::visit(ProgramNode& node) {
    for (const auto& item : node.items) {
        if (item) {
            item->accept(*this);
        }
    }
}

void MacroResolver::visit(ModDeclNode& node) {
    for (const auto& decl : node.decls) {
        if (decl) {
            decl->accept(*this);
        }
    }
}

void MacroResolver::visit(FunctionDeclNode& node) {
    if (node.body) {
        node.body->accept(*this);
    }
}

void MacroResolver::visit(ImplDeclNode& node) {
    for (const auto& method : node.methods) {
        if (method) {
            method->accept(*this);
        }
    }
}

void MacroResolver::visit(ExprStmtNode& node) {
    if (node.expr) {
        node.expr->accept(*this);
    }
}

void MacroResolver::visit(MacroDeclNode& node) {
    if (node.body) {
        node.body->accept(*this);
    }
}

void MacroResolver::visit(BlockStmtNode& node) {
    for (const auto& stmt : node.body) {
        if (stmt) {
            stmt->accept(*this);
        }
    }
}

void MacroResolver::visit(IfStmtNode& node) {
    if (node.condition) {
        node.condition->accept(*this);
    }
    if (node.thenBranch) {
        node.thenBranch->accept(*this);
    }
    if (node.elseBranch) {
        node.elseBranch->accept(*this);
    }
}

void MacroResolver::visit(WhileStmtNode& node) {
    if (node.condition) {
        node.condition->accept(*this);
    }
    if (node.body) {
        node.body->accept(*this);
    }
}

void MacroResolver::visit(ForStmtNode& node) {
    if (node.init) {
        node.init->accept(*this);
    }
    if (node.cond) {
        node.cond->accept(*this);
    }
    if (node.step) {
        node.step->accept(*this);
    }
    if (node.body) {
        node.body->accept(*this);
    }
}

void MacroResolver::visit(MacroCallExpr& node) {
    MacroID id = registry_.findMacroByName(node.name);
    std::cerr << "[DEBUG] MacroResolver resolved " << node.name << " to id " << id << std::endl;
    if (id == kInvalidMacroID) {
        diag_.error(node.loc, "Sử dụng macro chưa được định nghĩa: " + node.name + "!");
    } else {
        node.resolvedMacroId = id;
    }

    for (const auto& arg : node.args) {
        if (arg.node) {
            arg.node->accept(*this);
        }
    }
}

void MacroResolver::visit(MacroCallStmt& node) {
    MacroID id = registry_.findMacroByName(node.name);
    std::cerr << "[DEBUG] MacroResolver resolved MacroCallStmt " << node.name << " to id " << id << std::endl;
    if (id == kInvalidMacroID) {
        diag_.error(node.loc, "Sử dụng macro chưa được định nghĩa: " + node.name + "!");
    } else {
        node.resolvedMacroId = id;
    }

    for (const auto& arg : node.args) {
        if (arg.node) {
            arg.node->accept(*this);
        }
    }
}

void MacroResolver::visit(MacroExpandForStmt& node) {
    // Currently MacroExpandForStmt might not resolve a specific macro by name directly
    // in the same way, but it could contain other macro calls inside its body.
    if (node.body) {
        node.body->accept(*this);
    }
}

} // namespace fl




