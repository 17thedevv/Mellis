// =============================================================================
// mellis/MiddleEnd/ComptimeEvaluator.h
//
// ComptimeEvaluator - Evaluates AST nodes at compile-time.
// =============================================================================
#pragma once
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/Support/Diagnostic.h"
#include <variant>
#include <string>
#include <unordered_map>
#include <memory>

namespace fl {

class ExprNode;

// Represents a value evaluated at compile time.
struct ComptimeValue {
    enum class Kind {
        Void,
        Int,
        Float,
        Bool,
        String,
        Error
    };

    Kind kind = Kind::Void;
    union {
        int64_t iVal;
        double fVal;
        bool bVal;
    };
    std::string sVal; // For string literals

    ComptimeValue() : kind(Kind::Void) {}
    static ComptimeValue makeInt(int64_t v) { ComptimeValue val; val.kind = Kind::Int; val.iVal = v; return val; }
    static ComptimeValue makeFloat(double v) { ComptimeValue val; val.kind = Kind::Float; val.fVal = v; return val; }
    static ComptimeValue makeBool(bool v) { ComptimeValue val; val.kind = Kind::Bool; val.bVal = v; return val; }
    static ComptimeValue makeString(const std::string& v) { ComptimeValue val; val.kind = Kind::String; val.sVal = v; return val; }
    static ComptimeValue makeVoid() { return ComptimeValue(); }
    static ComptimeValue makeError() { ComptimeValue val; val.kind = Kind::Error; return val; }

    bool isError() const { return kind == Kind::Error; }
};

class ComptimeEnvironment {
public:
    ComptimeEnvironment(ComptimeEnvironment* parent = nullptr) : parent_(parent) {}

    void set(const std::string& name, const ComptimeValue& val) {
        variables_[name] = val;
    }
    
    ComptimeValue get(const std::string& name) const {
        auto it = variables_.find(name);
        if (it != variables_.end()) return it->second;
        if (parent_) return parent_->get(name);
        return ComptimeValue::makeError();
    }

    ComptimeEnvironment* parent_ = nullptr;
    std::unordered_map<std::string, ComptimeValue> variables_;
};

class ComptimeEvaluator : public ASTVisitor {
public:
    explicit ComptimeEvaluator(DiagnosticEngine& diag);

    bool evaluateComptimeBlocks(ASTNode* root);

    void visit(ProgramNode& node) override;
    
    // Visitor methods for evaluating nodes
    void visit(ComptimeStmtNode& node) override;
    void visit(BlockStmtNode& node) override;
    void visit(VarDeclNode& node) override;
    void visit(AssignExpr& node) override;
    void visit(ExprStmtNode& node) override;
    
    void visit(LiteralExpr& node) override;
    void visit(IdentifierExpr& node) override;
    void visit(BinaryExpr& node) override;
    
    void visit(FunctionDeclNode& node) override;
    
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
    void visit(IfStmtNode&) override {}
    void visit(WhileStmtNode&) override {}
    void visit(ForStmtNode&) override {}
    void visit(ReturnStmtNode&) override {}
    void visit(BreakStmtNode&) override {}
    void visit(ContinueStmtNode&) override {}
    void visit(UnsafeStmtNode&) override {}
    
    void visit(UnaryExpr&) override {}
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

    // Evaluate a specific expression node and return its value
    ComptimeValue evaluateExpr(ExprNode* expr);

private:
    DiagnosticEngine& diag_;
    ComptimeEnvironment* currentEnv_ = nullptr;
    ComptimeValue lastResult_; // Used to pass values back from visit()
    
    ComptimeValue popResult() {
        ComptimeValue res = lastResult_;
        lastResult_ = ComptimeValue::makeVoid();
        return res;
    }
};

} // namespace fl
