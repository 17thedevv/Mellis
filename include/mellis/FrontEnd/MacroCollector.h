#pragma once

#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class MacroCollector : public ASTVisitor {
public:
    MacroCollector(MacroRegistry& registry, DiagnosticEngine& diag)
        : registry_(registry), diag_(diag) {}

    // Start collection
    void collect(ProgramNode& program) {
        program.accept(*this);
    }

    // Top-level visitor
    void visit(ProgramNode& node) override;
    
    // Module visitor (to collect inside modules)
    void visit(ModDeclNode& node) override;

    // Macro declaration visitor
    void visit(MacroDeclNode& node) override;

    // Ignore other nodes
    void visit(VarDeclNode&) override {}
    void visit(FunctionDeclNode&) override {}
    void visit(ParamDeclNode&) override {}
    void visit(StructDeclNode&) override {}
    void visit(StructFieldNode&) override {}
    void visit(EnumDeclNode&) override {}
    void visit(EnumVariantNode&) override {}
    void visit(TraitDeclNode&) override {}
    void visit(ImplDeclNode&) override {}
    void visit(UseDeclNode&) override {}
    void visit(ExternDeclNode&) override {}
    void visit(TypeAliasDeclNode&) override {}

    void visit(BlockStmtNode&) override {}
    void visit(ExprStmtNode&) override {}
    void visit(IfStmtNode&) override {}
    void visit(WhileStmtNode&) override {}
    void visit(ForStmtNode&) override {}
    void visit(ReturnStmtNode&) override {}
    void visit(BreakStmtNode&) override {}
    void visit(ContinueStmtNode&) override {}
    void visit(UnsafeStmtNode&) override {}
    void visit(ComptimeStmtNode&) override {}

    void visit(LiteralExpr&) override {}
    void visit(IdentifierExpr&) override {}
    void visit(BinaryExpr&) override {}
    void visit(UnaryExpr&) override {}
    void visit(AssignExpr&) override {}
    void visit(CallExpr&) override {}
    void visit(MethodCallExpr&) override {}
    void visit(IndexExpr&) override {}
    void visit(MemberExpr&) override {}
    void visit(TupleIndexExpr&) override {}
    void visit(CastExpr&) override {}
    void visit(UnsizeCastExpr&) override {}
    void visit(ArrayLiteralExpr&) override {}
    void visit(TupleLiteralExpr&) override {}
    void visit(StructInitExpr&) override {}
    void visit(MatchExpr&) override {}
    void visit(LambdaExpr&) override {}
    void visit(AwaitExpr&) override {}
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}

    void visit(MacroCallExpr&) override {}
    void visit(MacroCallStmt&) override {}
    void visit(MacroExpandForStmt&) override {}
    void visit(PlaceholderExpr&) override {}
    void visit(PlaceholderStmt&) override {}




private:
    MacroRegistry& registry_;
    DiagnosticEngine& diag_;
};

} // namespace fl
