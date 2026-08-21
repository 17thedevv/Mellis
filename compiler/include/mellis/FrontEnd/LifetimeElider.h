#pragma once
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/Support/Diagnostic.h"
#include <vector>
#include <string>

namespace fl {

class ProgramNode;
class FunctionDeclNode;

class LifetimeElider : public ASTVisitor, public TypeVisitor {
    DiagnosticEngine& diag;
    uint32_t lifetimeCounter = 1;

    std::vector<LifetimeNode*> currentInputLifetimes;
    bool inInputParameters = false;

    std::string generateAnonymousLifetime();

public:
    explicit LifetimeElider(DiagnosticEngine& diag);

    void elide(ProgramNode& node);

    void visit(ProgramNode& node) override;
    
    // Declarations
    void visit(FunctionDeclNode& node) override;
    
    // We only need to visit Types inside Function parameters and returns.
    // The ASTVisitor automatically visits Types if we hook into param/return traversal, 
    // or we can manually invoke TypeVisitor.
    
    void visit(VarDeclNode&) override {}
    void visit(ParamDeclNode& node) override;
    void visit(StructDeclNode& node) override;
    void visit(EnumDeclNode& node) override;
    void visit(TraitDeclNode& node) override;
    void visit(ImplDeclNode& node) override;
    void visit(ModDeclNode& node) override;
    void visit(UseDeclNode&) override {}
    void visit(ExternDeclNode& node) override;
    void visit(TypeAliasDeclNode&) override {}
    void visit(EnumVariantNode&) override {}
    void visit(StructFieldNode&) override {}
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
    void visit(TryExpr&) override {}
    void visit(AwaitExpr&) override {}
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}
    void visit(TypeofExpr&) override {}

    // Types
    void visit(BuiltinTypeNode&) override {}
    void visit(NamedTypeNode& node) override;
    void visit(LifetimeNode& node) override;
    void visit(ReferenceTypeNode& node) override;
    void visit(PointerTypeNode& node) override;
    void visit(ArrayTypeNode& node) override;
    void visit(TupleTypeNode& node) override;
    void visit(FunctionTypeNode& node) override;
    void visit(NeverTypeNode&) override {}
    void visit(TraitObjectTypeNode& node) override;
};

} // namespace fl
