#pragma once

#include "mellis/AST/ASTNode.h"
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/TypeChecker.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

struct PatternAnalysisResult {
    bool isIrrefutable;
    std::string witness;
};

class MatchAnalyzer : public ASTVisitor {
    SymbolTable& table;
    TypeChecker& typeChecker;
    DiagnosticEngine& diag;
    bool hasError_ = false;

public:
    MatchAnalyzer(SymbolTable& table, TypeChecker& typeChecker, DiagnosticEngine& diag);
    bool analyze(ASTNode* root);
    PatternAnalysisResult checkIrrefutable(PatternNode* pat, const Type* expectedType);

    void visit(ProgramNode& node) override;
    void visit(ModDeclNode& node) override;
    void visit(ExternDeclNode& node) override {}
    void visit(VarDeclNode& node) override;
    void visit(FunctionDeclNode& node) override;
    void visit(StructDeclNode& node) override {}
    void visit(EnumDeclNode& node) override {}
    void visit(TraitDeclNode& node) override {}
    void visit(ImplDeclNode& node) override;
    void visit(TypeAliasDeclNode& node) override {}
    void visit(UseDeclNode& node) override {}
    void visit(ParamDeclNode& node) override {}
    void visit(StructFieldNode& node) override {}
    void visit(EnumVariantNode& node) override {}

    
    // Statements
    void visit(BlockStmtNode& node) override;
    void visit(ExprStmtNode& node) override;
    void visit(IfStmtNode& node) override;
    void visit(WhileStmtNode& node) override;
    void visit(ForStmtNode& node) override;
    void visit(ReturnStmtNode& node) override;
    void visit(BreakStmtNode& node) override {}
    void visit(ContinueStmtNode& node) override {}
    void visit(UnsafeStmtNode& node) override;
    void visit(ComptimeStmtNode& node) override;

    // Expressions
    void visit(MatchExpr& node) override;
    void visit(BinaryExpr& node) override;
    void visit(UnaryExpr& node) override;
    void visit(AssignExpr& node) override;
    void visit(CallExpr& node) override;
    void visit(MethodCallExpr& node) override;
    void visit(IndexExpr& node) override;
    void visit(MemberExpr& node) override;
    void visit(TupleIndexExpr& node) override;
    void visit(CastExpr& node) override;
    void visit(UnsizeCastExpr& node) override;
    void visit(ArrayLiteralExpr& node) override;
    void visit(TupleLiteralExpr& node) override;
    void visit(StructInitExpr& node) override;
    void visit(LambdaExpr& node) override;
    void visit(TryExpr& node) override;
    void visit(AwaitExpr& node) override;
    void visit(SizeofExpr& node) override {}
    void visit(AlignofExpr& node) override {}
    void visit(TypeofExpr& node) override {}
    void visit(LiteralExpr& node) override {}
    void visit(IdentifierExpr& node) override {}
};

} // namespace fl
