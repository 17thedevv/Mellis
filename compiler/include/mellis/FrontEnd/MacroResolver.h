#pragma once

#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/StmtNode.h"

namespace fl {

class MacroResolver : public ASTVisitor {
public:
    MacroResolver(MacroRegistry& registry, DiagnosticEngine& diag)
        : registry_(registry), diag_(diag) {}

    void resolve(ProgramNode& program) {
        program.accept(*this);
    }

    // Top-level visitor
    void visit(ProgramNode& node) override;
    
    // Visit declarations to traverse their bodies
    void visit(ModDeclNode& node) override;
    void visit(FunctionDeclNode& node) override;
    void visit(ImplDeclNode& node) override;
    
    // Visit statements to traverse nested scopes
    void visit(BlockStmtNode& node) override;
    void visit(IfStmtNode& node) override;
    void visit(WhileStmtNode& node) override;
    void visit(ForStmtNode& node) override;

    // The actual targets to resolve
    void visit(MacroCallExpr& node) override;
    void visit(MacroCallStmt& node) override;
    void visit(MacroExpandForStmt& node) override;


    void visit(VarDeclNode& node) override { std::cerr << "[DEBUG] MacroResolver visiting VarDeclNode\n"; if (node.initializer) node.initializer->accept(*this); }
    void visit(ParamDeclNode&) override {}
    void visit(StructDeclNode&) override {}
    void visit(StructFieldNode&) override {}
    void visit(EnumDeclNode&) override {}
    void visit(EnumVariantNode&) override {}
    void visit(TraitDeclNode&) override {}
    void visit(UseDeclNode&) override {}
    void visit(ExternDeclNode&) override {}
    void visit(TypeAliasDeclNode&) override {}

    void visit(ExprStmtNode&) override;
    void visit(ReturnStmtNode& node) override { if (node.value) node.value->accept(*this); }
    void visit(BreakStmtNode&) override {}
    void visit(ContinueStmtNode&) override {}
    void visit(UnsafeStmtNode&) override {}
    void visit(ComptimeStmtNode&) override {}

    void visit(LiteralExpr&) override {}
    void visit(IdentifierExpr&) override {}
    void visit(BinaryExpr& node) override { if (node.left) node.left->accept(*this); if (node.right) node.right->accept(*this); }
    void visit(UnaryExpr& node) override { if (node.operand) node.operand->accept(*this); }
    void visit(AssignExpr& node) override { if (node.lvalue) node.lvalue->accept(*this); if (node.value) node.value->accept(*this); }
    void visit(CallExpr& node) override { if (node.callee) node.callee->accept(*this); for (auto& arg : node.args) if (arg.value) arg.value->accept(*this); }
    void visit(MethodCallExpr& node) override { if (node.object) node.object->accept(*this); for (auto& arg : node.args) if (arg.value) arg.value->accept(*this); }
    void visit(IndexExpr& node) override { if (node.base) node.base->accept(*this); if (node.index) node.index->accept(*this); }
    void visit(MemberExpr& node) override { if (node.object) node.object->accept(*this); }
    void visit(TupleIndexExpr&) override {}
    void visit(CastExpr&) override {}
    void visit(UnsizeCastExpr&) override {}
    void visit(ArrayLiteralExpr&) override {}
    void visit(TupleLiteralExpr&) override {}
    void visit(StructInitExpr& node) override { for (auto& field : node.fields) if (field.value) field.value->accept(*this); }
    void visit(MatchExpr& node) override { if (node.subject) node.subject->accept(*this); for (auto& arm : node.arms) if (arm.body) arm.body->accept(*this); }
    void visit(LambdaExpr&) override {}
    void visit(TryExpr&) override {}
    void visit(AwaitExpr&) override {}
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}

    void visit(MacroDeclNode&) override;
    void visit(PlaceholderExpr&) override {}
    void visit(PlaceholderStmt&) override {}



private:
    MacroRegistry& registry_;
    DiagnosticEngine& diag_;
};

} // namespace fl
