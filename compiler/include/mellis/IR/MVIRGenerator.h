#pragma once

#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/TypeChecker.h"
#include "mellis/IR/MVIR.h"
#include "mellis/MiddleEnd/ABI/TraitObjectLayout.h"
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace fl {

class PatternNode;
struct DecisionNode;

class MVIRGenerator : public ASTVisitor {
    friend class PatternLowerer;
public:
    explicit MVIRGenerator(SymbolTable& symTable, DiagnosticEngine& diag, TypeChecker& typeChecker, std::unordered_map<const Type*, ClosureStorageKind>& storageMap);

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
    void visit(TupleIndexExpr& node) override;
    void visit(CastExpr& node) override;
    void visit(UnsizeCastExpr& node) override;
    void visit(ArrayLiteralExpr& node) override;
    void visit(TupleLiteralExpr& node) override;
    void visit(StructInitExpr& node) override;
    void visit(MatchExpr& node) override;
    void visit(LambdaExpr& node) override;
    void visit(TryExpr& node) override;
    void visit(AwaitExpr& node) override;
    void visit(SizeofExpr& node) override;
    void visit(AlignofExpr& node) override;
    void visit(TypeofExpr& node) override;

private:
    SymbolTable& table_;
    DiagnosticEngine& diag_;
    TypeChecker& typeChecker_;
    std::unordered_map<const Type*, ClosureStorageKind>& storageMap_;
    TraitObjectLayoutBuilder layoutBuilder_;

    bool isUnsafeContext_ = false;
    bool isComptimeContext_ = false;

    // Track which external symbols have been generated to avoid duplicates
    std::unordered_set<SymbolID> generatedExternals_;

    // ── Context State ────────────────────────────────────────────────────────

    std::unique_ptr<mvir::Module> module_;
    mvir::Function* currentFunction_ = nullptr;
    mvir::BasicBlock* currentBlock_ = nullptr;

    int nextLocalId_ = 0;
    int nextLabelId_ = 0;
    struct ScopeVar {
        mvir::LocalId id;
        const Type* type;
    };
    std::vector<std::vector<ScopeVar>> scopeStack_;
    
    void emitDropsForScopes(size_t targetDepth);
    

    std::unordered_map<SymbolID, mvir::LocalId> varAllocas_;
    std::unordered_set<SymbolID> borrowedCaptures_;

    struct LoopTarget {
        mvir::LabelId stepLbl;
        mvir::LabelId endLbl;
        size_t scopeDepth;
        ASTNode* loopNode;
    };
    std::vector<LoopTarget> loopTargets_;

    enum class EvalMode { RValue, LValue };
    EvalMode evalMode_ = EvalMode::RValue;

    mvir::Operand lastEvaluatedOperand_ = mvir::Number{"0"};

    // ── Helpers ──────────────────────────────────────────────────────────────

    mvir::Operand evaluateAutoRefReceiver(ExprNode& object, SymbolID methodId);

    mvir::LocalId nextLocal(uint32_t symbolId = 0, uint32_t expansionId = 0);
    mvir::LabelId nextLabel(const std::string& prefix = "bb");

    void terminateBlock(std::unique_ptr<mvir::Terminator> term);
    void startBlock(mvir::LabelId label);
    
    void pushLocalInst(std::unique_ptr<mvir::LocalInst> inst);
    
    void resetFunctionState();
    mvir::Operand evaluateLValue(ExprNode& expr);
    mvir::Operand evaluateRValue(ExprNode& expr);

    void compileDecisionTree(DecisionNode* node, std::unordered_map<std::string, mvir::Operand>& places, const std::vector<mvir::LabelId>& armLabels, mvir::LabelId fallbackLbl);
    void lowerPattern(PatternNode* pat, mvir::Operand sourceVal, const Type* sourceType);
    
    void emitCleanup(size_t targetScopeDepth);
};

} // namespace fl
