#include "mellis/FrontEnd/MacroValidator.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/PatternNode.h"

namespace fl {
void MacroValidator::visit(VarDeclNode& node) {
    if (node.typeAnnot) node.typeAnnot->accept(this->typeVisitor());
    if (node.initializer) node.initializer->accept(*this);
}

void MacroValidator::visit(ParamDeclNode& node) {
    if (node.type) node.type->accept(this->typeVisitor());
}

void MacroValidator::visit(FunctionDeclNode& node) {
    if (node.returnType) node.returnType->accept(this->typeVisitor());
    if (node.body) node.body->accept(*this);
    for (const auto& item : node.params) {
        if (item) item->accept(*this);
    }
    for (const auto& gp : node.genericParams) {
        for (const auto& bound : gp.bounds) {
            if (bound) bound->accept(this->typeVisitor());
        }
    }
}

void MacroValidator::visit(StructFieldNode& node) {
    if (node.type) node.type->accept(this->typeVisitor());
}

void MacroValidator::visit(StructDeclNode& node) {
    for (const auto& item : node.fields) {
        if (item) item->accept(*this);
    }
    for (const auto& gp : node.genericParams) {
        for (const auto& bound : gp.bounds) {
            if (bound) bound->accept(this->typeVisitor());
        }
    }
}

void MacroValidator::visit(EnumVariantNode& node) {
    for (const auto& item : node.fields) {
        if (item) item->accept(*this);
    }
}

void MacroValidator::visit(EnumDeclNode& node) {
    for (const auto& item : node.variants) {
        if (item) item->accept(*this);
    }
}

void MacroValidator::visit(TraitDeclNode& node) {
    for (const auto& item : node.methods) {
        if (item) item->accept(*this);
    }
    for (const auto& gp : node.genericParams) {
        for (const auto& bound : gp.bounds) {
            if (bound) bound->accept(this->typeVisitor());
        }
    }
}

void MacroValidator::visit(ImplDeclNode& node) {
    if (node.selfType) node.selfType->accept(this->typeVisitor());
    if (node.traitType) node.traitType->accept(this->typeVisitor());
    for (const auto& item : node.methods) {
        if (item) item->accept(*this);
    }
    for (const auto& gp : node.genericParams) {
        for (const auto& bound : gp.bounds) {
            if (bound) bound->accept(this->typeVisitor());
        }
    }
}

void MacroValidator::visit(ModDeclNode& node) {
    for (const auto& item : node.decls) {
        if (item) item->accept(*this);
    }
}

void MacroValidator::visit(UseDeclNode& node) {
}

void MacroValidator::visit(ExternDeclNode& node) {
    if (node.func) node.func->accept(*this);
}

void MacroValidator::visit(TypeAliasDeclNode& node) {
    if (node.aliasedType) node.aliasedType->accept(this->typeVisitor());
}

void MacroValidator::visit(LiteralExpr& node) {
}

void MacroValidator::visit(IdentifierExpr& node) {
    for (const auto& item : node.genericArgs) {
        if (item) item->accept(this->typeVisitor());
    }
}

void MacroValidator::visit(BinaryExpr& node) {
    if (node.left) node.left->accept(*this);
    if (node.right) node.right->accept(*this);
}

void MacroValidator::visit(UnaryExpr& node) {
    if (node.operand) node.operand->accept(*this);
}

void MacroValidator::visit(AssignExpr& node) {
    if (node.lvalue) node.lvalue->accept(*this);
    if (node.value) node.value->accept(*this);
}

void MacroValidator::visit(CallExpr& node) {
    if (node.callee) node.callee->accept(*this);
    for (const auto& item : node.genericArgs) {
        if (item) item->accept(this->typeVisitor());
    }
}

void MacroValidator::visit(MethodCallExpr& node) {
    if (node.object) node.object->accept(*this);
    for (const auto& item : node.genericArgs) {
        if (item) item->accept(this->typeVisitor());
    }
}

void MacroValidator::visit(IndexExpr& node) {
    if (node.base) node.base->accept(*this);
    if (node.index) node.index->accept(*this);
}

void MacroValidator::visit(MemberExpr& node) {
    if (node.object) node.object->accept(*this);
}

void MacroValidator::visit(TupleIndexExpr& node) {
    if (node.object) node.object->accept(*this);
}

void MacroValidator::visit(CastExpr& node) {
    if (node.expr) node.expr->accept(*this);
    if (node.targetType) node.targetType->accept(this->typeVisitor());
}

void MacroValidator::visit(UnsizeCastExpr& node) {
    if (node.expr) node.expr->accept(*this);
}

void MacroValidator::visit(ArrayLiteralExpr& node) {
    for (const auto& item : node.elements) {
        if (item) item->accept(*this);
    }
}

void MacroValidator::visit(TupleLiteralExpr& node) {
    for (const auto& item : node.elements) {
        if (item) item->accept(*this);
    }
}

void MacroValidator::visit(StructInitExpr& node) {
    for (const auto& item : node.genericArgs) {
        if (item) item->accept(this->typeVisitor());
    }
    for (const auto& field : node.fields) {
        if (field.value) field.value->accept(*this);
    }
}

void MacroValidator::visit(MatchExpr& node) {
    if (node.subject) node.subject->accept(*this);
    for (const auto& arm : node.arms) {
        if (arm.pattern) arm.pattern->accept(static_cast<PatternVisitor&>(*this));
        if (arm.body) arm.body->accept(*this);
    }
}

void MacroValidator::visit(PlaceholderExpr& node) {
}

void MacroValidator::visit(LambdaExpr& node) {
    if (node.returnType) node.returnType->accept(this->typeVisitor());
    if (node.body) node.body->accept(*this);
    for (const auto& item : node.params) {
        if (item) item->accept(*this);
    }
}

void MacroValidator::visit(AwaitExpr& node) {
    if (node.expr) node.expr->accept(*this);
}

void MacroValidator::visit(SizeofExpr& node) {
    if (node.targetType) node.targetType->accept(this->typeVisitor());
}

void MacroValidator::visit(AlignofExpr& node) {
    if (node.targetType) node.targetType->accept(this->typeVisitor());
}

void MacroValidator::visit(PlaceholderStmt& node) {
}

void MacroValidator::visit(BlockStmtNode& node) {
    for (const auto& item : node.body) {
        if (item) item->accept(*this);
    }
}

void MacroValidator::visit(ExprStmtNode& node) {
    if (node.expr) node.expr->accept(*this);
}

void MacroValidator::visit(IfStmtNode& node) {
    if (node.condition) node.condition->accept(*this);
    if (node.thenBranch) node.thenBranch->accept(*this);
    if (node.elseBranch) node.elseBranch->accept(*this);
}

void MacroValidator::visit(WhileStmtNode& node) {
    if (node.condition) node.condition->accept(*this);
    if (node.body) node.body->accept(*this);
}

void MacroValidator::visit(ForStmtNode& node) {
    if (node.iterable) node.iterable->accept(*this);
    if (node.init) node.init->accept(*this);
    if (node.cond) node.cond->accept(*this);
    if (node.step) node.step->accept(*this);
    if (node.body) node.body->accept(*this);
}

void MacroValidator::visit(ReturnStmtNode& node) {
    if (node.value) node.value->accept(*this);
}

void MacroValidator::visit(BreakStmtNode& node) {
}

void MacroValidator::visit(ContinueStmtNode& node) {
}

void MacroValidator::visit(UnsafeStmtNode& node) {
    if (node.body) node.body->accept(*this);
}

void MacroValidator::visit(ComptimeStmtNode& node) {
    if (node.body) node.body->accept(*this);
}

void MacroValidator::visit(BuiltinTypeNode& node) {
}

void MacroValidator::visit(NamedTypeNode& node) {
    for (const auto& item : node.genericArgs) {
        if (item) item->accept(this->typeVisitor());
    }
}

void MacroValidator::visit(ReferenceTypeNode& node) {
    if (node.inner) node.inner->accept(this->typeVisitor());
}

void MacroValidator::visit(PointerTypeNode& node) {
    if (node.inner) node.inner->accept(this->typeVisitor());
}

void MacroValidator::visit(ArrayTypeNode& node) {
    if (node.elementType) node.elementType->accept(this->typeVisitor());
    if (node.size) node.size->accept(*this);
}

void MacroValidator::visit(TupleTypeNode& node) {
    for (const auto& item : node.elements) {
        if (item) item->accept(this->typeVisitor());
    }
}

void MacroValidator::visit(FunctionTypeNode& node) {
    if (node.returnType) node.returnType->accept(this->typeVisitor());
    for (const auto& item : node.params) {
        if (item) item->accept(this->typeVisitor());
    }
}

void MacroValidator::visit(NeverTypeNode& node) {
}

void MacroValidator::visit(TraitObjectTypeNode& node) {
    if (node.trait) node.trait->accept(this->typeVisitor());
}

void MacroValidator::visit(PlaceholderTypeNode& node) {
}

void MacroValidator::visit(WildcardPatternNode& node) {
}

void MacroValidator::visit(LiteralPatternNode& node) {
    if (node.lit) node.lit->accept(*this);
}

void MacroValidator::visit(IdentifierPatternNode& node) {
}

void MacroValidator::visit(EnumPatternNode& node) {
    for (const auto& item : node.fields) {
        if (item) item->accept(static_cast<PatternVisitor&>(*this));
    }
}

void MacroValidator::visit(TuplePatternNode& node) {
    for (const auto& item : node.elements) {
        if (item) item->accept(static_cast<PatternVisitor&>(*this));
    }
}

void MacroValidator::visit(MacroDeclNode& node) {
    if (node.body) node.body->accept(*this);
}

void MacroValidator::visit(MacroCallExpr& node) {
    validateCall(node.resolvedMacroId, node.loc, node.args);
    for (auto& arg : node.args) {
        if (arg.node) arg.node->accept(*this);
    }
}

void MacroValidator::visit(MacroCallStmt& node) {
    validateCall(node.resolvedMacroId, node.loc, node.args);
    for (auto& arg : node.args) {
        if (arg.node) arg.node->accept(*this);
    }
}

void MacroValidator::visit(MacroExpandForStmt& node) {
    if (node.body) node.body->accept(*this);
}


void MacroValidator::validateCall(MacroID macroId, const SourceLocation& loc, const std::vector<MacroCallArgNode>& args) {
    if (macroId == kInvalidMacroID) return; // Already reported by MacroResolver

    const MacroDefinition* def = registry_.getMacro(macroId);
    if (!def || !def->templateAST) return;

    const auto& params = def->templateAST->params;
    
    // Validate argument count
    bool hasVariadic = !params.empty() && params.back().isVariadic;
    
    if (hasVariadic) {
        if (args.size() < params.size() - 1) {
            diag_.error(loc, "macro '" + def->templateAST->name + "' expected at least " + 
                std::to_string(params.size() - 1) + " arguments, found " + std::to_string(args.size()));
            return;
        }
    } else {
        if (args.size() != params.size()) {
            diag_.error(loc, "macro '" + def->templateAST->name + "' expected " + 
                std::to_string(params.size()) + " arguments, found " + std::to_string(args.size()));
            return;
        }
    }
    
    // Validate fragments
    for (size_t i = 0; i < args.size(); ++i) {
        size_t paramIdx = i < params.size() ? i : params.size() - 1;
        validateFragment(args[i], params[paramIdx].fragSpec, loc); // loc should ideally be arg loc
    }
}

void MacroValidator::validateFragment(const MacroCallArgNode& arg, MacroFragSpec expected, const SourceLocation& loc) {
    if (!arg.node) return;
    
    // Simple fragment validation
    // For now, if we expect Expr, it must be an ExprNode.
    // If Stmt, StmtNode, etc.
    
    switch (expected) {
        case MacroFragSpec::Expr:
            if (!dynamic_cast<ExprNode*>(arg.node.get())) {
                diag_.error(loc, "expected expression fragment");
            }
            break;
        case MacroFragSpec::Stmt:
            if (!dynamic_cast<StmtNode*>(arg.node.get())) {
                diag_.error(loc, "expected statement fragment");
            }
            break;
        case MacroFragSpec::Block:
            if (!dynamic_cast<BlockStmtNode*>(arg.node.get())) {
                diag_.error(loc, "expected block fragment");
            }
            break;
        case MacroFragSpec::Ident:
            if (!dynamic_cast<IdentifierExpr*>(arg.node.get()) && !dynamic_cast<IdentifierPatternNode*>(arg.node.get())) {
                diag_.error(loc, "expected identifier fragment");
            }
            break;
        case MacroFragSpec::Ty:
            if (!dynamic_cast<TypeNode*>(arg.node.get())) {
                diag_.error(loc, "expected type fragment");
            }
            break;
        default:
            break;
    }
}

void MacroValidator::visit(ProgramNode& node) {
    for (const auto& item : node.items) {
        if (item) item->accept(*this);
    }
}
} // namespace fl
