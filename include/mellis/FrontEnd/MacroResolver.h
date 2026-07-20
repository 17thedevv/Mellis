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


    void visit(VarDeclNode&) override {}
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

    void visit(MacroDeclNode&) override;
    void visit(PlaceholderExpr&) override {}
    void visit(PlaceholderStmt&) override {}



private:
    MacroRegistry& registry_;
    DiagnosticEngine& diag_;
};

} // namespace fl
