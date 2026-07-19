#pragma once

#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/TypeChecker.h"
#include "mellis/MiddleEnd/MVIR.h"
#include <unordered_map>

namespace fl {

class MVIRGenerator : public ASTVisitor {
public:
    explicit MVIRGenerator(SymbolTable& symTable, TypeChecker& typeChecker);

    /// Generate MVIR for the given program.
    std::unique_ptr<mvir::Module> generate(ProgramNode& program);

    void visit(ProgramNode& node) override;

    // Decl
    void visit(VarDeclNode& node) override;
    void visit(FunctionDeclNode& node) override;
    void visit(ParamDeclNode& node) override;
    void visit(StructDeclNode& node) override;
    void visit(StructFieldNode& node) override;
    void visit(EnumDeclNode& node) override;
    void visit(EnumVariantNode& node) override;
    void visit(TraitDeclNode& node) override;
    void visit(ImplDeclNode& node) override;
    void visit(ModDeclNode& node) override;
    void visit(UseDeclNode& node) override;
    void visit(ExternDeclNode& node) override;
    void visit(TypeAliasDeclNode& node) override;

    // Stmt
    void visit(BlockStmtNode& node) override;
    void visit(ExprStmtNode& node) override;
    void visit(IfStmtNode& node) override;
    void visit(WhileStmtNode& node) override;
    void visit(ForStmtNode& node) override;
    void visit(ReturnStmtNode& node) override;
    void visit(BreakStmtNode& node) override;
    void visit(ContinueStmtNode& node) override;
    void visit(UnsafeStmtNode& node) override;
    void visit(ComptimeStmtNode& node) override;

    // Expr
    void visit(LiteralExpr& node) override;
    void visit(IdentifierExpr& node) override;
    void visit(BinaryExpr& node) override;
    void visit(UnaryExpr& node) override;
    void visit(AssignExpr& node) override;
    void visit(CallExpr& node) override;
    void visit(MethodCallExpr& node) override;
    void visit(IndexExpr& node) override;
    void visit(MemberExpr& node) override;
    void visit(CastExpr& node) override;
    void visit(ArrayLiteralExpr& node) override;
    void visit(TupleLiteralExpr& node) override;
    void visit(StructInitExpr& node) override;
    void visit(MatchExpr& node) override;
    void visit(LambdaExpr& node) override;
    void visit(AwaitExpr& node) override;
    void visit(SizeofExpr& node) override;
    void visit(AlignofExpr& node) override;

private:
    SymbolTable& table_;
    TypeChecker& typeChecker_;

    // ── Context State ────────────────────────────────────────────────────────

    std::unique_ptr<mvir::Module> module_;
    mvir::Function* currentFunction_ = nullptr;
    mvir::BasicBlock* currentBlock_ = nullptr;

    int nextLocalId_ = 0;
    int nextLabelId_ = 0;
    std::vector<std::vector<mvir::LocalId>> scopeStack_;

    std::unordered_map<SymbolID, mvir::LocalId> varAllocas_;

    struct LoopTarget {
        mvir::LabelId stepLbl;
        mvir::LabelId endLbl;
    };
    std::vector<LoopTarget> loopTargets_;

    enum class EvalMode { RValue, LValue };
    EvalMode evalMode_ = EvalMode::RValue;

    mvir::Operand lastEvaluatedOperand_ = mvir::Number{"0"};

    // ── Helpers ──────────────────────────────────────────────────────────────

    mvir::LocalId nextLocal();
    mvir::LabelId nextLabel(const std::string& prefix = "bb");

    void terminateBlock(std::unique_ptr<mvir::Terminator> term);
    void startBlock(mvir::LabelId label);
    
    void resetFunctionState();
    mvir::Operand evaluateLValue(ExprNode& expr);
    mvir::Operand evaluateRValue(ExprNode& expr);
};

} // namespace fl
