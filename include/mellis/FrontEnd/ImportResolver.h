#pragma once

#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/MLib/ModuleLoader.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/ScopeStack.h"
#include <vector>
#include <string_view>

namespace fl {

class ImportResolver : public ASTVisitor {
public:
    ImportResolver(DiagnosticEngine& diag,
                   SymbolTable& symbolTable,
                   ModuleLoader& moduleLoader)
        : diag_(diag), symbolTable_(symbolTable), moduleLoader_(moduleLoader) {}

    void resolve(ProgramNode& program) {
        program.accept(*this);
    }

    void visit(ProgramNode& node) override;
    void visit(ModDeclNode& node) override;
    void visit(UseDeclNode& node) override;


    void visit(VarDeclNode&) override {}
    void visit(FunctionDeclNode&) override {}
    void visit(ParamDeclNode&) override {}
    void visit(StructDeclNode&) override {}
    void visit(StructFieldNode&) override {}
    void visit(EnumDeclNode&) override {}
    void visit(EnumVariantNode&) override {}
    void visit(TraitDeclNode&) override {}
    void visit(ImplDeclNode&) override {}
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

    void visit(MacroDeclNode&) override {}
    void visit(MacroCallExpr&) override {}
    void visit(MacroCallStmt&) override {}
    void visit(MacroExpandForStmt&) override {}
    void visit(PlaceholderExpr&) override {}
    void visit(PlaceholderStmt&) override {}



private:
    DiagnosticEngine& diag_;
    SymbolTable& symbolTable_;
    ModuleLoader& moduleLoader_;

    // Track the current module scope for path resolution
    // (mirrors the scope stack maintained by Resolver in Pass 1)
    ScopeID currentScope_ = 0; // global scope
};

} // namespace fl
