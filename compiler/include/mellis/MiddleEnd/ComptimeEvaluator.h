// =============================================================================
// mellis/MiddleEnd/ComptimeEvaluator.h
//
// ComptimeEvaluator - Evaluates AST nodes at compile-time.
// =============================================================================
#pragma once
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/IR/ConstantValue.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/FrontEnd/ASTVisitor.h"
#include <optional>
#include <unordered_map>
#include <string>

namespace fl {

class ExprNode;

class ComptimeEnvironment {
public:
    ComptimeEnvironment(ComptimeEnvironment* parent = nullptr) : parent_(parent) {}

    void set(const std::string& name, const mvir::ConstantValue& val) {
        variables_[name] = val;
    }
    
    mvir::ConstantValue get(const std::string& name) const {
        auto it = variables_.find(name);
        if (it != variables_.end()) return it->second;
        if (parent_) return parent_->get(name);
        return mvir::ConstantValue::makeError();
    }

    ComptimeEnvironment* parent_ = nullptr;
    std::unordered_map<std::string, mvir::ConstantValue> variables_;
};

class ComptimeEvaluator : public ASTVisitor {
public:
    explicit ComptimeEvaluator(DiagnosticEngine& diag, const SymbolTable& symbolTable);

    bool evaluateComptimeBlocks(ASTNode* root);

    void visit(ProgramNode& node) override;
    
    // Evaluate a specific expression node and return its value
    mvir::ConstantValue evaluateExpr(ExprNode* expr);
    
    void visit(ComptimeStmtNode& node) override;
    void visit(BlockStmtNode& node) override;
    void visit(VarDeclNode& node) override;
    void visit(AssignExpr& node) override;
    void visit(ExprStmtNode& node) override;
    
    void visit(LiteralExpr& node) override;
    void visit(IdentifierExpr& node) override;
    void visit(BinaryExpr& node) override;
    
    void visit(FunctionDeclNode& node) override;
    void visit(WhileStmtNode& node) override;
    
    // New implementations for comptime expression evaluation
    void visit(UnaryExpr& node) override;
    void visit(IfStmtNode& node) override;
    void visit(ArrayLiteralExpr& node) override;
    void visit(TupleLiteralExpr& node) override;
    
    // Dummy implementations for abstract methods
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
    void visit(ForStmtNode&) override {}
    void visit(ReturnStmtNode& node) override;
    void visit(BreakStmtNode& node) override;
    void visit(ContinueStmtNode& node) override;
    void visit(UnsafeStmtNode& node) override;
    
    void visit(CallExpr& node) override;
    void visit(MethodCallExpr&) override {}
    void visit(IndexExpr&) override {}
    void visit(MemberExpr&) override {}
    void visit(TupleIndexExpr&) override {}
    void visit(CastExpr&) override {}
    void visit(UnsizeCastExpr&) override {}
    void visit(StructInitExpr&) override {}
    void visit(MatchExpr&) override {}
    void visit(LambdaExpr&) override {}
    void visit(TryExpr&) override {}
    void visit(AwaitExpr&) override {}
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}
    void visit(TypeofExpr&) override {}

private:
    DiagnosticEngine& diag_;
    const SymbolTable& symbolTable_;
    ComptimeEnvironment* currentEnv_ = nullptr;
    mvir::ConstantValue lastResult_; // Used to pass values back from visit()
    
    // Control flow flags for comptime execution
    bool isExecuting_ = false;
    bool hasReturned_ = false;
    bool hasBroken_ = false;
    bool hasContinued_ = false;
    mvir::ConstantValue returnVal_;
    
    mvir::ConstantValue popResult() {
        mvir::ConstantValue res = lastResult_;
        lastResult_ = mvir::ConstantValue::makeVoid();
        return res;
    }
};

} // namespace fl
