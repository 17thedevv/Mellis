#pragma once
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/PatternNode.h"

namespace fl {

class EscapeAnalyzer : public ASTVisitor {
public:
    EscapeAnalyzer(TypeContext& typeCtx, SymbolTable& symTable, DiagnosticEngine& diag, 
                   std::unordered_map<const Type*, ClosureStorageKind>& storageMap)
        : typeCtx_(typeCtx), symTable_(symTable), diag_(diag), storageMap_(storageMap) {}

    void analyze(ASTNode* root);

    void visit(ProgramNode& node) override;
    void visit(FunctionDeclNode& node) override;
    void visit(BlockStmtNode& node) override;
    void visit(IfStmtNode& node) override;
    void visit(WhileStmtNode& node) override;
    void visit(MatchExpr& node) override;
    void visit(ReturnStmtNode& node) override;
    void visit(AssignExpr& node) override;
    void visit(CallExpr& node) override;
    void visit(VarDeclNode& node) override;
    void visit(LambdaExpr& node) override;

    // Stubs
    void visit(ParamDeclNode&) override {}
    void visit(StructDeclNode&) override {}
    void visit(StructFieldNode&) override {}
    void visit(EnumDeclNode&) override {}
    void visit(EnumVariantNode&) override {}
    void visit(TraitDeclNode&) override {}
    void visit(ImplDeclNode&) override {}
    void visit(ModDeclNode&) override {}
    void visit(UseDeclNode&) override {}
    void visit(ExternDeclNode&) override {}
    void visit(TypeAliasDeclNode&) override {}
    void visit(ExprStmtNode& node) override { if (node.expr) node.expr->accept(*this); }
    void visit(BreakStmtNode&) override {}
    void visit(ContinueStmtNode&) override {}
    void visit(UnsafeStmtNode&) override {}
    void visit(ComptimeStmtNode&) override {}
    void visit(ForStmtNode& node) override { if (node.body) node.body->accept(*this); }
    
    void visit(BinaryExpr& node) override { if(node.left) node.left->accept(*this); if(node.right) node.right->accept(*this); }
    void visit(UnaryExpr& node) override { if(node.operand) node.operand->accept(*this); }
    void visit(MethodCallExpr&) override {}
    void visit(IndexExpr&) override {}
    void visit(MemberExpr& node) override { if(node.object) node.object->accept(*this); }
    void visit(TupleIndexExpr&) override {}
    void visit(CastExpr& node) override { if(node.expr) node.expr->accept(*this); }
    void visit(UnsizeCastExpr& node) override { if(node.expr) node.expr->accept(*this); }
    void visit(ArrayLiteralExpr& node) override { for(auto& e:node.elements) e->accept(*this); }
    void visit(TupleLiteralExpr& node) override { for(auto& e:node.elements) e->accept(*this); }
    void visit(StructInitExpr& node) override { for(auto& f:node.fields) f.value->accept(*this); }
    void visit(TryExpr& node) override { if(node.expr) node.expr->accept(*this); }
    void visit(AwaitExpr& node) override { if(node.expr) node.expr->accept(*this); }
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}
    void visit(LiteralExpr&) override {}
    void visit(IdentifierExpr&) override {}

private:
    TypeContext& typeCtx_;
    SymbolTable& symTable_;
    DiagnosticEngine& diag_;
    std::unordered_map<const Type*, ClosureStorageKind>& storageMap_;
    
    // Track lambda nodes by their symbol/type to build the closure graph
    void markEscape(ExprNode* expr);
    void markNestedClosuresEscaping(const Type* type, SourceLocation loc);
    void markClosureEscaping(ClosureType* closureTy, SourceLocation loc);
    void propagateNestedCaptures(ClosureType* closureTy, SourceLocation loc);
    void checkBorrowEscape(const ClosureType* closureTy, SourceLocation loc);
};

} // namespace fl
