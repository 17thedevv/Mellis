#include "fdlang/MiddleEnd/MatchAnalyzer.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/StmtNode.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/AST/ProgramNode.h"
#include "fdlang/AST/PatternNode.h"
#include "fdlang/Core/FLType.h"
#include <unordered_set>
#include <iostream>
#include <string>

namespace fl {

// Helper visitor to extract variant IDs covered by a pattern
class CoveredVariantVisitor : public PatternVisitor {
public:
    std::unordered_set<SymbolID> coveredVariants;
    bool hasCatchAll = false;

    void visit(WildcardPatternNode&) override { hasCatchAll = true; }
    void visit(LiteralPatternNode&) override {}
    void visit(IdentifierPatternNode&) override { hasCatchAll = true; }
    void visit(TuplePatternNode& node) override {
        // For tuples, exhaustiveness is more complex. We skip deep tuple checking for MVP.
    }
    void visit(EnumPatternNode& node) override {
        if (node.variantSymbolId != kInvalidSymbolID) {
            coveredVariants.insert(node.variantSymbolId);
        }
    }
};

MatchAnalyzer::MatchAnalyzer(SymbolTable& table, DiagnosticEngine& diag)
    : table(table), diag(diag) {}

bool MatchAnalyzer::analyze(ASTNode* root) {
    if (!root) return false;
    root->accept(*this);
    return true; // We don't track errors as a boolean return here, diag tracks it
}

void MatchAnalyzer::visit(ProgramNode& node) { for (auto& item : node.items) item->accept(*this); }
void MatchAnalyzer::visit(ModDeclNode& node) { for (auto& d : node.decls) d->accept(*this); }
void MatchAnalyzer::visit(VarDeclNode& node) { if (node.initializer) node.initializer->accept(*this); }
void MatchAnalyzer::visit(FunctionDeclNode& node) { if (node.body) node.body->accept(*this); }
void MatchAnalyzer::visit(ImplDeclNode& node) { for (auto& m : node.methods) m->accept(*this); }

// Statements
void MatchAnalyzer::visit(BlockStmtNode& node) { for (auto& s : node.body) s->accept(*this); }
void MatchAnalyzer::visit(ExprStmtNode& node) { node.expr->accept(*this); }
void MatchAnalyzer::visit(IfStmtNode& node) {
    node.condition->accept(*this);
    node.thenBranch->accept(*this);
    if (node.elseBranch) node.elseBranch->accept(*this);
}
void MatchAnalyzer::visit(WhileStmtNode& node) {
    node.condition->accept(*this);
    node.body->accept(*this);
}
void MatchAnalyzer::visit(ForStmtNode& node) {
    if (node.init) node.init->accept(*this);
    if (node.cond) node.cond->accept(*this);
    if (node.step) node.step->accept(*this);
    if (node.iterable) node.iterable->accept(*this);
    node.body->accept(*this);
}
void MatchAnalyzer::visit(ReturnStmtNode& node) { if (node.value) node.value->accept(*this); }
void MatchAnalyzer::visit(UnsafeStmtNode& node) { if (node.body) node.body->accept(*this); }
void MatchAnalyzer::visit(ComptimeStmtNode& node) { if (node.body) node.body->accept(*this); }

// Expressions
void MatchAnalyzer::visit(MatchExpr& node) {
    node.subject->accept(*this);

    CoveredVariantVisitor visitor;
    for (auto& arm : node.arms) {
        if (arm.pattern) arm.pattern->accept(visitor);
        if (arm.body) arm.body->accept(*this);
    }

    if (node.subject->inferredType) {
        if (auto* enumType = dynamic_cast<const EnumType*>(node.subject->inferredType)) {
            auto sym = table.getSymbol(enumType->enumSymbolId);
            if (sym.kind == SymbolKind::Enum && sym.decl) {
                auto* enumDecl = static_cast<EnumDeclNode*>(sym.decl);
                if (!visitor.hasCatchAll) {
                    std::vector<std::string_view> missing;
                    for (auto& variant : enumDecl->variants) {
                        if (visitor.coveredVariants.find(variant->symbolId) == visitor.coveredVariants.end()) {
                            missing.push_back(variant->name);
                        }
                    }
                    if (!missing.empty()) {
                        std::string missingStr = "";
                        for (size_t i = 0; i < missing.size(); ++i) {
                            missingStr += "`" + std::string(missing[i]) + "`";
                            if (i < missing.size() - 1) missingStr += ", ";
                        }
                        diag.error(node.loc, "non-exhaustive patterns: " + missingStr + " not covered");
                    }
                }
            }
        } else if (auto* primType = dynamic_cast<const PrimitiveType*>(node.subject->inferredType)) {
            if (!visitor.hasCatchAll && primType->builtinKind != BuiltinKind::Void) {
                diag.error(node.loc, "non-exhaustive patterns: `_` not covered");
            }
        }
    }
}

void MatchAnalyzer::visit(BinaryExpr& node) { node.left->accept(*this); node.right->accept(*this); }
void MatchAnalyzer::visit(UnaryExpr& node) { node.operand->accept(*this); }
void MatchAnalyzer::visit(AssignExpr& node) { node.lvalue->accept(*this); node.value->accept(*this); }
void MatchAnalyzer::visit(CallExpr& node) {
    node.callee->accept(*this);
    for (auto& arg : node.args) if (arg.value) arg.value->accept(*this);
}
void MatchAnalyzer::visit(MethodCallExpr& node) {
    node.object->accept(*this);
    for (auto& arg : node.args) if (arg.value) arg.value->accept(*this);
}
void MatchAnalyzer::visit(IndexExpr& node) { node.base->accept(*this); node.index->accept(*this); }
void MatchAnalyzer::visit(MemberExpr& node) { node.object->accept(*this); }
void MatchAnalyzer::visit(CastExpr& node) { node.expr->accept(*this); }
void MatchAnalyzer::visit(ArrayLiteralExpr& node) { for (auto& e : node.elements) e->accept(*this); }
void MatchAnalyzer::visit(TupleLiteralExpr& node) { for (auto& e : node.elements) e->accept(*this); }
void MatchAnalyzer::visit(StructInitExpr& node) { for (auto& f : node.fields) f.value->accept(*this); }
void MatchAnalyzer::visit(LambdaExpr& node) { if (node.body) node.body->accept(*this); }
void MatchAnalyzer::visit(AwaitExpr& node) { node.expr->accept(*this); }

} // namespace fl
